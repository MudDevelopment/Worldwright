use crate::{
    OidcConfig,
    auth::{AuthError, AuthService},
};
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) struct PendingOidc {
    pub verifier: PkceCodeVerifier,
    pub nonce: Nonce,
    pub expires: Instant,
    pub invite_hash: Option<[u8; 32]>,
}

#[derive(Debug)]
pub(crate) enum OidcFinishError {
    Invalid,
    Unauthorized,
    Unavailable,
    Internal,
}

#[derive(Clone)]
pub(crate) struct OidcService {
    config: OidcConfig,
    metadata: CoreProviderMetadata,
    auth: AuthService,
    pending: Arc<Mutex<HashMap<String, PendingOidc>>>,
    http: reqwest::Client,
}

pub(crate) struct OidcStart {
    pub location: String,
}

impl OidcService {
    pub async fn discover(
        config: OidcConfig,
        auth: AuthService,
    ) -> Result<Self, crate::ServerError> {
        let issuer = IssuerUrl::new(config.issuer.clone())
            .map_err(|_| crate::ServerError::Configuration("OIDC issuer is invalid"))?;
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10));
        // Fixture trust exists only in the test binary; never disable TLS verification.
        #[cfg(test)]
        let http = if let Ok(path) = std::env::var("WORLDWRIGHT_R2_CA") {
            let pem = std::fs::read(path)
                .map_err(|_| crate::ServerError::startup("test CA unavailable"))?;
            let certificate = reqwest::Certificate::from_pem(&pem)
                .map_err(|_| crate::ServerError::startup("test CA invalid"))?;
            http.no_proxy().add_root_certificate(certificate)
        } else {
            http
        };
        let http = http
            .build()
            .map_err(|_| crate::ServerError::startup("OIDC HTTP client failed"))?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|_| crate::ServerError::startup("OIDC provider discovery failed"))?;
        if metadata.issuer().as_str() != config.issuer {
            return Err(crate::ServerError::startup("OIDC issuer mismatch"));
        }
        Ok(Self {
            config,
            metadata,
            auth,
            pending: Arc::new(Mutex::new(HashMap::new())),
            http,
        })
    }
    #[cfg(test)]
    pub(crate) fn for_r2_test(config: OidcConfig, metadata: CoreProviderMetadata) -> Self {
        Self {
            config,
            metadata,
            auth: AuthService::from_records_for_test(vec![], Duration::from_secs(600)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            http: reqwest::ClientBuilder::new()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_millis(50))
                .build()
                .unwrap(),
        }
    }
    #[cfg(test)]
    pub(crate) fn assert_r2_pending(&self, state: &str, nonce: &str, challenge: &str) {
        let pending = self.pending.lock().unwrap();
        let record = pending.get(state).unwrap();
        assert_eq!(record.nonce.secret(), nonce);
        assert_eq!(
            PkceCodeChallenge::from_code_verifier_sha256(&record.verifier).as_str(),
            challenge
        );
    }
    #[cfg(test)]
    pub(crate) fn expire_r2_pending(&self, state: &str) {
        self.pending.lock().unwrap().get_mut(state).unwrap().expires = Instant::now();
    }
    pub fn start(&self, invite: Option<String>) -> Result<OidcStart, AuthError> {
        let invite_hash = invite
            .as_ref()
            .map(|v| worldwright_persistence::oidc_token_hash(v));
        if invite.as_ref().is_some_and(|v| {
            v.len() != 64
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }) {
            return Err(AuthError::InvalidInput);
        }
        let redirect = RedirectUrl::new(self.config.redirect_uri.clone())
            .map_err(|_| AuthError::InvalidInput)?;
        let client = CoreClient::from_provider_metadata(
            self.metadata.clone(),
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        )
        .set_redirect_uri(redirect);
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .set_pkce_challenge(challenge)
            .add_scope(Scope::new("openid".into()))
            .url();
        let mut pending = self.pending.lock().map_err(|_| AuthError::Internal)?;
        pending.retain(|_, p| p.expires > Instant::now());
        if pending.len() >= 128 {
            return Err(AuthError::Saturated);
        }
        pending.insert(
            state.secret().to_owned(),
            PendingOidc {
                verifier,
                nonce,
                expires: Instant::now() + Duration::from_secs(300),
                invite_hash,
            },
        );
        Ok(OidcStart {
            location: url.to_string(),
        })
    }
    pub async fn finish(&self, code: String, state: String) -> Result<String, OidcFinishError> {
        if code.is_empty()
            || code.len() > 2048
            || state.is_empty()
            || state.len() > 256
            || code.chars().any(char::is_control)
            || state.chars().any(char::is_control)
        {
            return Err(OidcFinishError::Invalid);
        }
        let pending = self
            .pending
            .lock()
            .map_err(|_| OidcFinishError::Internal)?
            .remove(&state)
            .ok_or(OidcFinishError::Invalid)?;
        if pending.expires <= Instant::now() {
            return Err(OidcFinishError::Invalid);
        }
        // Refresh provider metadata/JWKS for every one-shot callback so key
        // rotation cannot strand an otherwise valid login.
        let issuer =
            IssuerUrl::new(self.config.issuer.clone()).map_err(|_| OidcFinishError::Internal)?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &self.http)
            .await
            .map_err(|_| OidcFinishError::Unavailable)?;
        if metadata.issuer().as_str() != self.config.issuer {
            return Err(OidcFinishError::Unauthorized);
        }
        let redirect = RedirectUrl::new(self.config.redirect_uri.clone())
            .map_err(|_| OidcFinishError::Internal)?;
        let client = CoreClient::from_provider_metadata(
            metadata.clone(),
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        )
        .set_redirect_uri(redirect);
        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|_| OidcFinishError::Invalid)?
            .set_pkce_verifier(pending.verifier)
            .request_async(&self.http)
            .await
            .map_err(|_| OidcFinishError::Unavailable)?;
        let id = token
            .extra_fields()
            .id_token()
            .ok_or(OidcFinishError::Unauthorized)?;
        let claims = id
            .claims(&client.id_token_verifier(), &pending.nonce)
            .map_err(|_| OidcFinishError::Unauthorized)?;
        let subject = claims.subject().as_str();
        if subject.is_empty() || subject.len() > 255 || subject.chars().any(char::is_control) {
            return Err(OidcFinishError::Unauthorized);
        }
        let invite = pending.invite_hash;
        self.auth
            .oidc_login(metadata.issuer().as_str(), subject, invite)
            .await
            .map_err(|e| match e {
                AuthError::Unauthorized | AuthError::InvalidInput => OidcFinishError::Unauthorized,
                AuthError::Saturated | AuthError::SessionCapacity => OidcFinishError::Unavailable,
                _ => OidcFinishError::Internal,
            })
    }
}
