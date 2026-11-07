use crate::{
    ServerError,
    cli::ReleaseCommand,
    release_runtime::{ReleaseRuntimeError, verify_and_smoke},
};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
use worldwright_core::ids::{
    ActivationId, CorrelationId, OperationId, PublicationId, WorldId, WriterId,
};
use worldwright_operations::{OperatorClaims, decode_hex_key, verify_token};
use worldwright_persistence::{
    ActivationPlan, OperatorContext, PersistenceError, PostgresStore, PublicationRecord,
};
use worldwright_release::verify_bytes;

pub(crate) enum ReleaseRunError {
    Conflict,
    Message(String),
}

impl std::fmt::Display for ReleaseRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => formatter.write_str("release conflicts with durable state"),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl From<ServerError> for ReleaseRunError {
    fn from(error: ServerError) -> Self {
        Self::Message(error.to_string())
    }
}

pub(crate) async fn run(command: ReleaseCommand) -> Result<(), ReleaseRunError> {
    match command {
        ReleaseCommand::Request {
            auth,
            artifact,
            kind,
            target,
            expected_current,
        } => {
            let store = connect(&auth.database_env).await?;
            let claims = load_claims(&auth)?;
            if target_environment(target) != auth.environment {
                return Err(ReleaseRunError::Message(
                    "target environment conflicts with operator scope".into(),
                ));
            }
            let bytes = read_artifact(&artifact)?;
            let verified = verify_bytes("release request artifact", &bytes, None)
                .map_err(|error| ReleaseRunError::Message(error.to_string()))?;
            let request = random_id::<PublicationId>()?;
            let context = make_context(&auth, &claims, random_id::<CorrelationId>()?)?;
            let record = store
                .submit_publication_authorized(
                    &context,
                    request,
                    &bytes,
                    kind,
                    target,
                    expected_current,
                    random_id::<OperationId>()?,
                )
                .await
                .map_err(map_error)?;
            print_record(&record);
            println!("Artifact digest: {}", verified.artifact_digest());
            Ok(())
        }
        ReleaseCommand::Review {
            auth,
            request,
            decision,
        } => {
            let store = connect(&auth.database_env).await?;
            let claims = load_claims(&auth)?;
            let context = make_context(&auth, &claims, random_id::<CorrelationId>()?)?;
            let record = store
                .review_publication_authorized(
                    &context,
                    request,
                    decision,
                    random_id::<OperationId>()?,
                )
                .await
                .map_err(map_error)?;
            print_record(&record);
            Ok(())
        }
        ReleaseCommand::Status { auth, request } => {
            let store = connect(&auth.database_env).await?;
            let claims = load_claims(&auth)?;
            let context = make_context(&auth, &claims, random_id::<CorrelationId>()?)?;
            let record = store
                .publication_authorized(&context, request)
                .await
                .map_err(map_error)?;
            print_record(&record);
            Ok(())
        }
        ReleaseCommand::Activate { auth, request } => {
            let store = connect(&auth.database_env).await?;
            let claims = load_claims(&auth)?;
            let context = make_context(&auth, &claims, random_id::<CorrelationId>()?)?;
            let plan = store
                .begin_activation_authorized(
                    &context,
                    request,
                    random_id::<ActivationId>()?,
                    random_id::<WriterId>()?,
                    random_id::<OperationId>()?,
                )
                .await
                .map_err(map_error)?;
            finish_activation(&store, &auth, claims, plan).await
        }
        ReleaseCommand::Recover { auth } => {
            let store = connect(&auth.database_env).await?;
            let claims = load_claims(&auth)?;
            let context = make_context(&auth, &claims, random_id::<CorrelationId>()?)?;
            let plan = store
                .recover_activation_authorized(
                    &context,
                    random_id::<WriterId>()?,
                    random_id::<OperationId>()?,
                )
                .await
                .map_err(map_error)?;
            finish_activation(&store, &auth, claims, plan).await
        }
    }
}

fn load_claims(auth: &crate::cli::ReleaseAuth) -> Result<OperatorClaims, ReleaseRunError> {
    let token = std::env::var(&auth.token_env).map_err(|_| {
        ReleaseRunError::Message("operator token environment is unavailable".into())
    })?;
    let key_text = std::env::var(&auth.key_env)
        .map_err(|_| ReleaseRunError::Message("operator key environment is unavailable".into()))?;
    let key = decode_hex_key(&key_text)
        .map_err(|_| ReleaseRunError::Message("operator key is invalid".into()))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ReleaseRunError::Message("clock is unavailable".into()))?
        .as_secs();
    verify_token(&token, &key, &auth.issuer, &auth.audience, now)
        .map_err(|_| ReleaseRunError::Message("operator token is invalid".into()))
}
pub(crate) fn load_claims_public(auth: &crate::cli::ReleaseAuth) -> Result<OperatorClaims, String> {
    load_claims(auth).map_err(|e| e.to_string())
}

fn make_context<'a>(
    auth: &'a crate::cli::ReleaseAuth,
    claims: &'a OperatorClaims,
    correlation: CorrelationId,
) -> Result<OperatorContext<'a>, ReleaseRunError> {
    Ok(OperatorContext {
        claims,
        environment: auth.environment,
        world: world_id(),
        correlation,
        reason: &auth.reason,
    })
}
pub(crate) fn make_context_public<'a>(
    auth: &'a crate::cli::ReleaseAuth,
    claims: &'a OperatorClaims,
) -> Result<OperatorContext<'a>, String> {
    let correlation = random_id::<CorrelationId>().map_err(|e| e.to_string())?;
    make_context(auth, claims, correlation).map_err(|e| e.to_string())
}

fn target_environment(
    target: worldwright_release::DeploymentTarget,
) -> worldwright_operations::Environment {
    match target {
        worldwright_release::DeploymentTarget::Staging => {
            worldwright_operations::Environment::Staging
        }
        worldwright_release::DeploymentTarget::Production => {
            worldwright_operations::Environment::Production
        }
    }
}

fn world_id() -> WorldId {
    WorldId::from_u128(1).expect("fixed world ID is nonzero")
}

async fn connect(env_name: &str) -> Result<PostgresStore, ReleaseRunError> {
    let url = std::env::var(env_name).map_err(|_| {
        ReleaseRunError::Message("database URL environment variable is unavailable".into())
    })?;
    if url.is_empty() {
        return Err(ReleaseRunError::Message(
            "database URL environment variable is unavailable".into(),
        ));
    }
    let store = PostgresStore::connect(&url)
        .await
        .map_err(|_| ReleaseRunError::Message("database connection failed".into()))?;
    store
        .verify_migrations()
        .await
        .map_err(|_| ReleaseRunError::Message("database migrations are not current".into()))?;
    Ok(store)
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, ReleaseRunError> {
    let metadata = fs::metadata(path)
        .map_err(|_| ReleaseRunError::Message("release artifact cannot be read".into()))?;
    if !metadata.is_file() {
        return Err(ReleaseRunError::Message(
            "release artifact is not a regular file".into(),
        ));
    }
    let file = File::open(path)
        .map_err(|_| ReleaseRunError::Message("release artifact cannot be read".into()))?;
    let mut bytes = Vec::new();
    file.take((worldwright_release::MAX_ARTIFACT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ReleaseRunError::Message("release artifact cannot be read".into()))?;
    if bytes.len() > worldwright_release::MAX_ARTIFACT_BYTES {
        return Err(ReleaseRunError::Message(
            "release artifact exceeds size limit".into(),
        ));
    }
    Ok(bytes)
}

async fn finish_activation(
    store: &PostgresStore,
    auth: &crate::cli::ReleaseAuth,
    claims: OperatorClaims,
    plan: ActivationPlan,
) -> Result<(), ReleaseRunError> {
    let context = make_context(auth, &claims, random_id::<CorrelationId>()?)?;
    match verify_and_smoke(store, &plan).await {
        Ok(()) => {
            store
                .complete_activation_authorized(&context, plan.clone(), random_id::<OperationId>()?)
                .await
                .map_err(map_error)?;
            println!("Request: {}", plan.request());
            println!("Status: activated");
            println!("Release ID: {}", plan.artifact().release_id());
            println!("Artifact digest: {}", plan.artifact().digest());
            Ok(())
        }
        Err(ReleaseRuntimeError::Incompatible) => {
            fail_after_check(
                store,
                auth,
                claims,
                plan,
                "durable_incompatible",
                "activation compatibility check failed",
            )
            .await
        }
        Err(ReleaseRuntimeError::Smoke) => {
            fail_after_check(
                store,
                auth,
                claims,
                plan,
                "smoke_failed",
                "activation smoke check failed",
            )
            .await
        }
        Err(ReleaseRuntimeError::Persistence) => Err(ReleaseRunError::Message(
            "activation compatibility persistence check failed".into(),
        )),
    }
}

async fn fail_after_check(
    store: &PostgresStore,
    auth: &crate::cli::ReleaseAuth,
    claims: OperatorClaims,
    plan: ActivationPlan,
    code: &str,
    message: &str,
) -> Result<(), ReleaseRunError> {
    let context = make_context(auth, &claims, random_id::<CorrelationId>()?)?;
    store
        .fail_activation_authorized(&context, plan, code, random_id::<OperationId>()?)
        .await
        .map_err(|_| ReleaseRunError::Message("activation failure cleanup is uncertain".into()))?;
    Err(ReleaseRunError::Message(message.into()))
}

fn print_record(record: &PublicationRecord) {
    println!("Request: {}", record.request());
    println!("World: {}", record.world());
    println!("Release ID: {}", record.release_id());
    println!("Artifact digest: {}", record.artifact_digest());
    println!("Kind: {}", record.kind());
    println!("Target: {}", record.target());
    println!("Status: {}", record.status());
    println!("Requested by: {}", record.requested_by());
    if let Some(expected) = record.expected_current() {
        println!("Expected current: {expected}");
    } else {
        println!("Expected current: none");
    }
    if let Some(value) = record.reviewed_by() {
        println!("Reviewed by: {value}");
    }
    if let Some(value) = record.review_reason() {
        println!("Review reason: {value}");
    }
    if let Some(value) = record.activation_executor() {
        println!("Activation executor: {value}");
    }
}

fn map_error(error: PersistenceError) -> ReleaseRunError {
    match error {
        PersistenceError::ReleaseConflict | PersistenceError::ExpectedReleaseConflict => {
            ReleaseRunError::Conflict
        }
        other => ReleaseRunError::Message(other.to_string()),
    }
}

fn random_id<T>() -> Result<T, ReleaseRunError>
where
    T: RandomInstanceId,
{
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)
            .map_err(|_| ReleaseRunError::Message("random identity generation failed".into()))?;
        if let Some(value) = T::from_bytes(bytes) {
            return Ok(value);
        }
    }
    Err(ReleaseRunError::Message(
        "random identity generation failed".into(),
    ))
}

trait RandomInstanceId: Sized {
    fn from_bytes(bytes: [u8; 16]) -> Option<Self>;
}

macro_rules! random_instance_id {
    ($($type:ty),+ $(,)?) => {
        $(impl RandomInstanceId for $type {
            fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
                <$type>::from_u128(u128::from_be_bytes(bytes)).ok()
            }
        })+
    };
}
random_instance_id!(
    PublicationId,
    ActivationId,
    WriterId,
    CorrelationId,
    OperationId
);
