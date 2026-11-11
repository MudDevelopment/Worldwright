//! Controlled signed-token and pending-flow units; NOT live provider/browser proof.
use crate::oidc::*;
use openidconnect::{
    ClientId, IssuerUrl, Nonce,
    core::{CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreProviderMetadata},
};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/r2-oidc-verifier.json")).unwrap()
}

#[test]
fn r2_signed_claims_reject_bad_issuer_audience_nonce_signature_and_expiry() {
    let data = fixture();
    let keys: CoreJsonWebKeySet = serde_json::from_value(data["jwks_initial"].clone()).unwrap();
    let verifier = CoreIdTokenVerifier::new_public_client(
        ClientId::new(data["audience"].as_str().unwrap().into()),
        IssuerUrl::new(data["issuer"].as_str().unwrap().into()).unwrap(),
        keys,
    );
    for name in [
        "valid",
        "wrong_issuer",
        "wrong_audience",
        "wrong_nonce",
        "invalid_signature",
        "expired",
    ] {
        let token: CoreIdToken = serde_json::from_value(data["tokens"][name].clone()).unwrap();
        let result = token.claims(
            &verifier,
            &Nonce::new(data["nonce"].as_str().unwrap().into()),
        );
        assert_eq!(result.is_ok(), name == "valid", "{name}");
    }
}

#[test]
fn r2_rotated_key_requires_refreshed_key_material() {
    let data = fixture();
    let token: CoreIdToken = serde_json::from_value(data["tokens"]["rotated"].clone()).unwrap();
    for (field, expected) in [("jwks_initial", false), ("jwks_rotated", true)] {
        let keys: CoreJsonWebKeySet = serde_json::from_value(data[field].clone()).unwrap();
        let verifier = CoreIdTokenVerifier::new_public_client(
            ClientId::new(data["audience"].as_str().unwrap().into()),
            IssuerUrl::new(data["issuer"].as_str().unwrap().into()).unwrap(),
            keys,
        );
        assert_eq!(
            token
                .claims(
                    &verifier,
                    &Nonce::new(data["nonce"].as_str().unwrap().into())
                )
                .is_ok(),
            expected
        );
    }
}

fn service(issuer: String) -> OidcService {
    let metadata: CoreProviderMetadata = serde_json::from_value(json!({
        "issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),
        "token_endpoint":format!("{issuer}/token"),"jwks_uri":format!("{issuer}/jwks"),
        "response_types_supported":["code"],"subject_types_supported":["public"],
        "id_token_signing_alg_values_supported":["RS256"]
    }))
    .unwrap();
    OidcService::for_r2_test(
        crate::OidcConfig {
            issuer,
            client_id: "r2-client".into(),
            client_secret: "test-only".into(),
            redirect_uri: "https://client.example.invalid/callback".into(),
        },
        metadata,
    )
}

#[test]
fn r2_start_binds_unique_nonce_state_and_exact_s256_verifier() {
    let service = service("https://issuer.example.invalid".into());
    let a = service.start(None).unwrap();
    let b = service.start(None).unwrap();
    let a: std::collections::HashMap<_, _> = reqwest::Url::parse(&a.location)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    let b: std::collections::HashMap<_, _> = reqwest::Url::parse(&b.location)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    assert_ne!(a["state"], b["state"]);
    assert_ne!(a["nonce"], b["nonce"]);
    service.assert_r2_pending(&a["state"], &a["nonce"], &a["code_challenge"]);
    assert_eq!(a["code_challenge_method"], "S256");
    assert!(service.start(Some("invalid invite".into())).is_err());
}

#[tokio::test]
async fn r2_expired_pending_state_and_provider_outage_are_one_shot() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let issuer = format!("https://{}", listener.local_addr().unwrap());
    drop(listener);
    let service = service(issuer);
    let url = reqwest::Url::parse(&service.start(None).unwrap().location).unwrap();
    let state = url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    assert!(matches!(
        service.finish("valid-code".into(), state.clone()).await,
        Err(OidcFinishError::Unavailable)
    ));
    assert!(matches!(
        service.finish("valid-code".into(), state).await,
        Err(OidcFinishError::Invalid)
    ));
    let url = reqwest::Url::parse(&service.start(None).unwrap().location).unwrap();
    let state = url
        .query_pairs()
        .find(|(name, _)| name == "state")
        .unwrap()
        .1
        .into_owned();
    service.expire_r2_pending(&state);
    assert!(matches!(
        service.finish("valid-code".into(), state.clone()).await,
        Err(OidcFinishError::Invalid)
    ));
    assert!(matches!(
        service.finish("valid-code".into(), state).await,
        Err(OidcFinishError::Invalid)
    ));
}
