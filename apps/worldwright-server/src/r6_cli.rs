//! R6 explicit maintenance commands. No automatic scheduling or broad deletion.
use crate::{ServerError, cli::ReleaseAuth};
use std::{collections::BTreeMap, ffi::OsString};
use worldwright_core::ids::{OperationId, RecoveryJobId};

pub(crate) enum Maintenance {
    Status(ReleaseAuth),
    Prune { auth: ReleaseAuth, batch: usize, operation: OperationId },
    Interrupt { auth: ReleaseAuth, job: RecoveryJobId, operation: OperationId },
}

pub(crate) fn parse(args: impl IntoIterator<Item=OsString>) -> Result<Maintenance, String> {
    let mut args = args.into_iter();
    let sub = args.next().and_then(|v| v.into_string().ok()).ok_or("maintenance subcommand required")?;
    let mut values = BTreeMap::new();
    let mut confirmed = false;
    let mut stopped = false;
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "invalid maintenance option")?;
        if key == "--confirm-worker-stopped" {
            if stopped { return Err("duplicate worker confirmation".into()); }
            stopped = true;
            continue;
        }
        if key == "--confirm-expired-authentication-only" {
            if confirmed { return Err("duplicate confirmation".into()); }
            confirmed = true;
            continue;
        }
        let value = args.next().and_then(|v| v.into_string().ok()).ok_or("maintenance option value required")?;
        if values.insert(key, value).is_some() { return Err("duplicate maintenance option".into()); }
    }
    let common = ["--database-url-env", "--operator-token-env", "--operator-key-env",
        "--operator-issuer", "--operator-audience", "--environment", "--reason"];
    for key in values.keys() {
        if !common.contains(&key.as_str()) && !match sub.as_str() {
            "retention-prune" => matches!(key.as_str(), "--batch" | "--operation"),
            "interrupt-job" => matches!(key.as_str(), "--job" | "--operation"),
            _ => false,
        } { return Err("unknown or cross-command maintenance option".into()); }
    }
    if !matches!(sub.as_str(), "retention-status" | "retention-prune" | "interrupt-job") {
        return Err("maintenance retention-status|retention-prune|interrupt-job required".into());
    }
    if confirmed != (sub == "retention-prune") { return Err("retention-prune requires --confirm-expired-authentication-only".into()); }
    if stopped != (sub == "interrupt-job") { return Err("interrupt-job requires --confirm-worker-stopped; stop/isolate the actual worker first".into()); }
    let required = |name: &str| values.get(name).filter(|s| !s.is_empty()).cloned().ok_or_else(|| format!("{name} required"));
    let env_name = |name: &str| -> Result<String, String> {
        let value = required(name)?;
        let bytes = value.as_bytes();
        if bytes.len() > 128 || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
            || bytes[1..].iter().any(|v| !v.is_ascii_alphanumeric() && *v != b'_') {
            return Err("invalid environment-variable name".into());
        }
        Ok(value)
    };
    let auth = ReleaseAuth { database_env: env_name("--database-url-env")?,
        token_env: env_name("--operator-token-env")?, key_env: env_name("--operator-key-env")?,
        issuer: required("--operator-issuer")?, audience: required("--operator-audience")?,
        environment: serde_json::from_value(serde_json::Value::String(required("--environment")?))
            .map_err(|_| "environment must be staging or production")?,
        reason: required("--reason")? };
    if auth.reason.len()>512 || auth.reason.trim()!=auth.reason || auth.reason.chars().any(char::is_control) {
        return Err("reason must be 1..512 UTF-8 bytes without surrounding whitespace/control characters".into());
    }
    match sub.as_str() {
        "retention-status" => Ok(Maintenance::Status(auth)),
        "retention-prune" => {
            let batch = required("--batch")?.parse::<usize>().map_err(|_| "invalid batch")?;
            if !(1..=worldwright_persistence::MAX_RETENTION_BATCH).contains(&batch) { return Err("batch must be 1..1000".into()); }
            let operation = required("--operation")?.parse().map_err(|_| "invalid operation ID")?;
            Ok(Maintenance::Prune { auth, batch, operation })
        }
        _ => Ok(Maintenance::Interrupt { auth,
            job: required("--job")?.parse().map_err(|_| "invalid job ID")?,
            operation: required("--operation")?.parse().map_err(|_| "invalid operation ID")? }),
    }
}

pub(crate) async fn run(command: Maintenance) -> Result<(), ServerError> {
    let auth = match &command {
        Maintenance::Status(auth) | Maintenance::Prune { auth, .. } | Maintenance::Interrupt { auth, .. } => auth,
    };
    let url = std::env::var(&auth.database_env).map_err(|_| ServerError::startup("maintenance database unavailable"))?;
    let store = worldwright_persistence::PostgresStore::connect(&url).await
        .map_err(|_| ServerError::startup("maintenance connection failed"))?;
    store.verify_migrations().await.map_err(|_| ServerError::startup("current migrations required for maintenance"))?;
    let claims = crate::release_cli::load_claims_public(auth).map_err(|_| ServerError::startup("operator token invalid"))?;
    let context = crate::release_cli::make_context_public(auth, &claims).map_err(|_| ServerError::startup("operator context invalid"))?;
    match &command {
        Maintenance::Status(_) => {
            let report = store.retention_status(&context).await.map_err(|_| ServerError::startup("retention status denied or unavailable"))?;
            println!("{}", serde_json::to_string(&report).map_err(|_| ServerError::startup("report serialization failed"))?);
        }
        Maintenance::Prune { batch, operation, .. } => {
            println!("retention operation: {operation}");
            let report = store.prune_expired_authentication(&context, *batch, *operation).await
                .map_err(|_| ServerError::startup("retention denied, failed or commit uncertain; retain operation ID and retry only the same intent"))?;
            println!("{}", serde_json::to_string(&report).map_err(|_| ServerError::startup("report serialization failed"))?);
        }
        Maintenance::Interrupt { job, operation, .. } => {
            store.interrupt_recovery_job(&context, *job, *operation).await
                .map_err(|_| ServerError::startup("job reconciliation denied, conflicted or commit uncertain; inspect job/audit before retry"))?;
            println!("job marked failed; no PostgreSQL tool rerun or takeover performed");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base(sub: &str) -> Vec<OsString> {
        [sub,"--database-url-env","DB","--operator-token-env","TOKEN","--operator-key-env","KEY",
            "--operator-issuer","issuer","--operator-audience","audience","--environment","staging","--reason","maintenance"]
            .into_iter().map(OsString::from).collect()
    }
    #[test]
    fn r6_maintenance_requires_explicit_bounded_intent() {
        assert!(parse(base("retention-status")).is_ok());
        assert!(parse(base("retention-prune")).is_err());
        let mut values = base("retention-prune");
        values.extend(["--batch","1000","--operation","00000000000000000000000000000001","--confirm-expired-authentication-only"].into_iter().map(OsString::from));
        assert!(parse(values.clone()).is_ok());
        values.extend(["--batch","1"].into_iter().map(OsString::from));
        assert!(parse(values).is_err());
        let mut invalid = base("retention-status");
        invalid.push("--confirm-expired-authentication-only".into());
        assert!(parse(invalid).is_err());
        assert!(parse(base("unknown")).is_err());
    }
}
