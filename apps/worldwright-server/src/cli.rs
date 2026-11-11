use crate::release_cli::{ReleaseRunError, run as run_release};
use crate::{BoundServer, ServerConfig, ServerError, StorageConfig};
use axum::http::Uri;
use sha2::{Digest, Sha256};
use std::{
    env,
    ffi::OsString,
    fs::OpenOptions,
    fs::{self, File},
    io::{self, Read, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    process::{Command as ProcessCommand, ExitCode, Stdio},
    str::FromStr,
    thread,
    time::{Duration, Instant},
};
use worldwright_operations::{BackupManifest, sign_backup_manifest, verify_backup_manifest};
use worldwright_operations::{Capability, Environment, decode_hex_key};
use worldwright_release::{DeploymentTarget, PublicationKind, ReviewDecision, Sha256Digest};

const MAX_ARGUMENTS: usize = 32;

pub(crate) enum Command {
    Help,
    Version,
    Status,
    HashPassword,
    Serve(ServeOptions),
    Database(DatabaseCommand),
    Release(ReleaseCommand),
    Operator(OperatorCommand),
    Backup(BackupCommand),
    Takeover(TakeoverOptions),
}

#[cfg(test)]
mod backup_parser_tests {
    use super::*;

    #[test]
    fn backup_rejects_cross_command_and_duplicate_special_options() {
        let base = [
            "backup",
            "verify",
            "--input",
            "x",
            "--manifest",
            "m",
            "--backup-signing-key-env",
            "KEY",
        ];
        assert!(parse(base.iter().map(OsString::from)).is_ok());
        for extra in [
            vec!["--pg-dump", "tool"],
            vec!["--pg-dump", "a", "--pg-dump", "b"],
            vec!["--confirm-empty-target", "db"],
        ] {
            let mut values = base.to_vec();
            values.extend(extra);
            assert!(parse(values.iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn takeover_requires_confirmation() {
        let args = ["recovery", "takeover", "--database-url-env", "DB"];
        assert!(parse(args.iter().map(OsString::from)).is_err());
    }
}

pub(crate) struct ServeOptions {
    pub(crate) bind: SocketAddr,
    pub(crate) allowed_origin: String,
    pub(crate) storage: ParsedStorage,
    pub(crate) allow_insecure_http: bool,
    pub(crate) behind_tls_ingress: bool,
    pub(crate) operator_key_env: Option<String>,
    pub(crate) operator_issuer: Option<String>,
    pub(crate) operator_audience: Option<String>,
    pub(crate) environment: Option<Environment>,
    pub(crate) game: crate::GameProfile,
    pub(crate) oidc: Option<ParsedOidcConfig>,
}

pub(crate) struct ParsedOidcConfig {
    issuer: String,
    client_id: String,
    client_secret_env: String,
    redirect_uri: String,
}

pub(crate) enum ParsedStorage {
    Local {
        accounts_path: PathBuf,
        content_path: PathBuf,
    },
    Postgres {
        env_name: String,
    },
}

pub(crate) enum DatabaseCommand {
    Migrate {
        env_name: String,
    },
    Seed {
        env_name: String,
        content: PathBuf,
        accounts: PathBuf,
        game: worldwright_release::GameContract,
    },
    OidcInvite {
        env_name: String,
        username: String,
        issuer: String,
        ttl_seconds: u64,
    },
}

pub(crate) enum ReleaseCommand {
    Request {
        auth: ReleaseAuth,
        artifact: PathBuf,
        kind: PublicationKind,
        target: DeploymentTarget,
        expected_current: Option<Sha256Digest>,
    },
    Review {
        auth: ReleaseAuth,
        request: worldwright_core::ids::PublicationId,
        decision: ReviewDecision,
    },
    Status {
        auth: ReleaseAuth,
        request: worldwright_core::ids::PublicationId,
    },
    Activate {
        auth: ReleaseAuth,
        request: worldwright_core::ids::PublicationId,
    },
    Recover {
        auth: ReleaseAuth,
    },
}

pub(crate) struct ReleaseAuth {
    pub(crate) database_env: String,
    pub(crate) token_env: String,
    pub(crate) key_env: String,
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) environment: Environment,
    pub(crate) reason: String,
}

pub(crate) enum OperatorCommand {
    Bootstrap {
        env_name: String,
        subject: String,
        display_name: String,
        kind: worldwright_operations::OperatorKind,
        environment: Environment,
        capabilities: Vec<Capability>,
        reason: String,
    },
}

pub(crate) enum BackupCommand {
    Verify {
        input: PathBuf,
        manifest: PathBuf,
        key_env: String,
    },
    Create {
        database_env: String,
        auth: ReleaseAuth,
        service: String,
        output: PathBuf,
        key_env: String,
        dump: Option<PathBuf>,
    },
    #[allow(dead_code)]
    Restore {
        control_env: String,
        target_env: String,
        auth: ReleaseAuth,
        input: PathBuf,
        manifest: PathBuf,
        key_env: String,
        service: String,
        restore: Option<PathBuf>,
        confirm: String,
    },
}
#[allow(dead_code)]
pub(crate) struct TakeoverOptions {
    pub(crate) auth: ReleaseAuth,
    pub(crate) confirm: bool,
}

pub(crate) fn parse<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut bounded = Vec::with_capacity(MAX_ARGUMENTS);
    let mut arguments = arguments.into_iter();
    for _ in 0..MAX_ARGUMENTS {
        if let Some(argument) = arguments.next() {
            bounded.push(argument);
        } else {
            break;
        }
    }
    if arguments.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    parse_inner(bounded)
}

fn parse_inner(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Err("a command is required".to_owned());
    };
    match command.to_str() {
        Some("--help" | "-h") if arguments.next().is_none() => Ok(Command::Help),
        Some("--version" | "-V") if arguments.next().is_none() => Ok(Command::Version),
        Some("status") if arguments.next().is_none() => Ok(Command::Status),
        Some("hash-password") if arguments.next().is_none() => Ok(Command::HashPassword),
        Some("database") => parse_database(arguments),
        Some("serve") => parse_serve(arguments),
        Some("release") => parse_release(arguments),
        Some("operator") => parse_operator(arguments),
        Some("backup") => parse_backup(arguments),
        Some("recovery") => parse_takeover(arguments),
        Some(_) => Err("unknown command or extra argument".to_owned()),
        None => Err("command must be valid Unicode".to_owned()),
    }
}

fn parse_backup<I: IntoIterator<Item = OsString>>(arguments: I) -> Result<Command, String> {
    let mut a = arguments.into_iter();
    let sub = a
        .next()
        .ok_or("backup subcommand is required")?
        .into_string()
        .map_err(|_| "backup subcommand invalid")?;
    let mut v = std::collections::HashMap::<String, String>::new();
    let mut dump = None;
    let mut restore = None;
    let mut confirm = None;
    while let Some(x) = a.next() {
        let k = x.into_string().map_err(|_| "backup option invalid")?;
        if k == "--pg-dump" {
            if dump.is_some() {
                return Err("duplicate backup option".into());
            }
            dump = Some(next_path(&mut a, "--pg-dump")?)
        } else if k == "--pg-restore" {
            if restore.is_some() {
                return Err("duplicate backup option".into());
            }
            restore = Some(next_path(&mut a, "--pg-restore")?)
        } else if k == "--confirm-empty-target" {
            if confirm.is_some() {
                return Err("duplicate backup option".into());
            }
            confirm = Some(next_semantic(&mut a, "--confirm-empty-target")?)
        } else {
            let val = next_semantic(&mut a, &k)?;
            if v.insert(k, val).is_some() {
                return Err("duplicate backup option".into());
            }
        }
    }
    let allowed: &[&str] = match sub.as_str() {
        "verify" => &["--input", "--manifest", "--backup-signing-key-env"],
        "create" => &[
            "--database-url-env",
            "--operator-token-env",
            "--operator-key-env",
            "--operator-issuer",
            "--operator-audience",
            "--environment",
            "--reason",
            "--pg-service",
            "--output",
            "--backup-signing-key-env",
        ],
        "restore" => &[
            "--control-database-url-env",
            "--target-database-url-env",
            "--operator-token-env",
            "--operator-key-env",
            "--operator-issuer",
            "--operator-audience",
            "--environment",
            "--reason",
            "--input",
            "--manifest",
            "--backup-signing-key-env",
            "--pg-service",
        ],
        _ => return Err("backup create|verify|restore is required".into()),
    };
    if v.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("invalid backup option".into());
    }
    let req = |k: &str| v.get(k).cloned().ok_or_else(|| format!("{k} is required"));
    match sub.as_str() {
        "verify" => {
            if dump.is_some() || restore.is_some() || confirm.is_some() {
                return Err("invalid backup option".into());
            }
            Ok(Command::Backup(BackupCommand::Verify {
                input: PathBuf::from(req("--input")?),
                manifest: PathBuf::from(req("--manifest")?),
                key_env: req("--backup-signing-key-env")?,
            }))
        }
        "create" => {
            if restore.is_some() || confirm.is_some() {
                return Err("invalid backup option".into());
            }
            let auth = ReleaseAuth {
                database_env: req("--database-url-env")?,
                token_env: req("--operator-token-env")?,
                key_env: req("--operator-key-env")?,
                issuer: req("--operator-issuer")?,
                audience: req("--operator-audience")?,
                environment: serde_json::from_value(serde_json::Value::String(req(
                    "--environment",
                )?))
                .map_err(|_| "invalid environment")?,
                reason: req("--reason")?,
            };
            Ok(Command::Backup(BackupCommand::Create {
                database_env: auth.database_env.clone(),
                auth,
                service: req("--pg-service")?,
                output: PathBuf::from(req("--output")?),
                key_env: req("--backup-signing-key-env")?,
                dump,
            }))
        }
        "restore" => {
            if dump.is_some() {
                return Err("invalid backup option".into());
            }
            let auth = ReleaseAuth {
                database_env: req("--control-database-url-env")?,
                token_env: req("--operator-token-env")?,
                key_env: req("--operator-key-env")?,
                issuer: req("--operator-issuer")?,
                audience: req("--operator-audience")?,
                environment: serde_json::from_value(serde_json::Value::String(req(
                    "--environment",
                )?))
                .map_err(|_| "invalid environment")?,
                reason: req("--reason")?,
            };
            Ok(Command::Backup(BackupCommand::Restore {
                control_env: auth.database_env.clone(),
                target_env: req("--target-database-url-env")?,
                auth,
                input: PathBuf::from(req("--input")?),
                manifest: PathBuf::from(req("--manifest")?),
                key_env: req("--backup-signing-key-env")?,
                service: req("--pg-service")?,
                restore,
                confirm: confirm.ok_or("--confirm-empty-target is required")?,
            }))
        }
        _ => unreachable!("subcommand was validated above"),
    }
}
fn parse_takeover<I: IntoIterator<Item = OsString>>(arguments: I) -> Result<Command, String> {
    let mut a = arguments.into_iter();
    if a.next().and_then(|v| v.into_string().ok()).as_deref() != Some("takeover") {
        return Err("recovery takeover is required".into());
    }
    let mut v = std::collections::HashMap::new();
    let mut confirm = false;
    while let Some(x) = a.next() {
        let k = x.into_string().map_err(|_| "option invalid")?;
        if k == "--confirm-old-writers-isolated" {
            if confirm {
                return Err("duplicate option".into());
            }
            confirm = true
        } else {
            let z = next_semantic(&mut a, &k)?;
            if v.insert(k, z).is_some() {
                return Err("duplicate option".into());
            }
        }
    }
    if !confirm {
        return Err("--confirm-old-writers-isolated is required".into());
    }
    const ALLOWED: &[&str] = &[
        "--database-url-env",
        "--operator-token-env",
        "--operator-key-env",
        "--operator-issuer",
        "--operator-audience",
        "--environment",
        "--reason",
    ];
    if v.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err("invalid recovery option".into());
    }
    let req = |k: &str| v.get(k).cloned().ok_or_else(|| format!("{k} is required"));
    let auth = ReleaseAuth {
        database_env: req("--database-url-env")?,
        token_env: req("--operator-token-env")?,
        key_env: req("--operator-key-env")?,
        issuer: req("--operator-issuer")?,
        audience: req("--operator-audience")?,
        environment: serde_json::from_value(serde_json::Value::String(req("--environment")?))
            .map_err(|_| "invalid environment")?,
        reason: req("--reason")?,
    };
    Ok(Command::Takeover(TakeoverOptions { auth, confirm }))
}

fn parse_operator<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = arguments.into_iter();
    let subcommand = args
        .next()
        .ok_or_else(|| "operator subcommand is required".to_owned())?;
    if subcommand.to_str() != Some("bootstrap") {
        return Err("invalid operator subcommand".into());
    }
    let mut env_name = None;
    let mut subject = None;
    let mut display_name = None;
    let mut kind = None;
    let mut environment = None;
    let mut capabilities = None;
    let mut reason = None;
    let mut confirmed = false;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--database-url-env") if env_name.is_none() => {
                env_name = Some(next_env_name(&mut args)?)
            }
            Some("--subject") if subject.is_none() => {
                subject = Some(next_semantic(&mut args, "--subject")?)
            }
            Some("--display-name") if display_name.is_none() => {
                display_name = Some(next_semantic(&mut args, "--display-name")?)
            }
            Some("--kind") if kind.is_none() => {
                kind = Some(
                    serde_json::from_value(serde_json::Value::String(next_semantic(
                        &mut args, "--kind",
                    )?))
                    .map_err(|_| "invalid operator kind".to_owned())?,
                )
            }
            Some("--environment") if environment.is_none() => {
                environment = Some(
                    serde_json::from_value(serde_json::Value::String(next_semantic(
                        &mut args,
                        "--environment",
                    )?))
                    .map_err(|_| "invalid environment".to_owned())?,
                )
            }
            Some("--capabilities") if capabilities.is_none() => {
                let value = next_semantic(&mut args, "--capabilities")?;
                let values = value
                    .split(',')
                    .map(|item| {
                        serde_json::from_value(serde_json::Value::String(item.to_owned()))
                            .map_err(|_| "invalid capability".to_owned())
                    })
                    .collect::<Result<Vec<Capability>, _>>()?;
                capabilities = Some(values);
            }
            Some("--reason") if reason.is_none() => {
                reason = Some(next_semantic(&mut args, "--reason")?)
            }
            Some("--confirm-first-operator") if !confirmed => confirmed = true,
            Some(_) => return Err("invalid or duplicate operator option".into()),
            None => return Err("operator options must be Unicode".into()),
        }
    }
    if !confirmed {
        return Err("--confirm-first-operator is required".into());
    }
    Ok(Command::Operator(OperatorCommand::Bootstrap {
        env_name: env_name.ok_or("--database-url-env is required")?,
        subject: subject.ok_or("--subject is required")?,
        display_name: display_name.ok_or("--display-name is required")?,
        kind: kind.ok_or("--kind is required")?,
        environment: environment.ok_or("--environment is required")?,
        capabilities: capabilities.ok_or("--capabilities is required")?,
        reason: reason.ok_or("--reason is required")?,
    }))
}

fn parse_database<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = arguments.into_iter();
    let subcommand = args
        .next()
        .ok_or_else(|| "database subcommand is required".to_owned())?;
    let subcommand = subcommand
        .to_str()
        .ok_or_else(|| "database subcommand must be Unicode".to_owned())?;
    let mut env_name = None;
    let mut content = None;
    let mut accounts = None;
    let mut game = None;
    let mut username = None;
    let mut issuer = None;
    let mut ttl_seconds = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--database-url-env") if env_name.is_none() => {
                env_name = Some(next_env_name(&mut args)?)
            }
            Some("--content") if content.is_none() => {
                content = Some(next_path(&mut args, "--content")?)
            }
            Some("--accounts") if accounts.is_none() => {
                accounts = Some(next_path(&mut args, "--accounts")?)
            }
            Some("--game") if game.is_none() => {
                game = Some(match next_unicode(&mut args, "--game")?.as_str() {
                    "reference" => worldwright_release::GameContract::Reference,
                    "expedition" => worldwright_release::GameContract::Expedition,
                    _ => return Err("--game must be reference or expedition".to_owned()),
                })
            }
            Some("--username") if username.is_none() => {
                username = Some(next_semantic(&mut args, "--username")?)
            }
            Some("--issuer") if issuer.is_none() => {
                issuer = Some(next_semantic(&mut args, "--issuer")?)
            }
            Some("--ttl-seconds") if ttl_seconds.is_none() => {
                ttl_seconds = Some(
                    next_unicode(&mut args, "--ttl-seconds")?
                        .parse::<u64>()
                        .map_err(|_| "--ttl-seconds must be an integer".to_owned())?,
                )
            }
            _ => return Err("invalid database option".to_owned()),
        }
    }
    let env_name = env_name.ok_or_else(|| "--database-url-env is required".to_owned())?;
    match subcommand {
        "migrate"
            if content.is_none()
                && accounts.is_none()
                && game.is_none()
                && username.is_none()
                && issuer.is_none()
                && ttl_seconds.is_none() =>
        {
            Ok(Command::Database(DatabaseCommand::Migrate { env_name }))
        }
        "seed" if username.is_none() && issuer.is_none() && ttl_seconds.is_none() => {
            Ok(Command::Database(DatabaseCommand::Seed {
                env_name,
                content: content.ok_or_else(|| "--content is required".to_owned())?,
                accounts: accounts.ok_or_else(|| "--accounts is required".to_owned())?,
                game: game.unwrap_or(worldwright_release::GameContract::Reference),
            }))
        }
        "oidc-invite" if content.is_none() && accounts.is_none() && game.is_none() => {
            Ok(Command::Database(DatabaseCommand::OidcInvite {
                env_name,
                username: username.ok_or_else(|| "--username is required".to_owned())?,
                issuer: issuer.ok_or_else(|| "--issuer is required".to_owned())?,
                ttl_seconds: ttl_seconds.unwrap_or(86_400),
            }))
        }
        _ => Err("invalid database subcommand/options".to_owned()),
    }
}

fn next_env_name<I>(arguments: &mut I) -> Result<String, String>
where
    I: Iterator<Item = OsString>,
{
    let value = next_unicode(arguments, "--database-url-env")?;
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_'
        || bytes[1..]
            .iter()
            .any(|b| !b.is_ascii_alphanumeric() && *b != b'_')
    {
        return Err("environment name is invalid".to_owned());
    }
    Ok(value)
}

fn parse_serve<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut arguments = arguments.into_iter();
    let mut content = None;
    let mut accounts = None;
    let mut database_env = None;
    let mut acquire_writer = false;
    let mut origin = None;
    let mut bind = None;
    let mut allow_insecure_http = false;
    let mut behind_tls_ingress = false;
    let mut operator_key_env = None;
    let mut operator_issuer = None;
    let mut operator_audience = None;
    let mut environment = None;
    let mut game = None;
    let mut oidc_issuer: Option<String> = None;
    let mut oidc_client_id: Option<String> = None;
    let mut oidc_secret_env: Option<String> = None;
    let mut oidc_redirect: Option<String> = None;
    while let Some(argument) = arguments.next() {
        let option = argument
            .to_str()
            .ok_or_else(|| "serve options must be valid Unicode".to_owned())?;
        match option {
            "--content" => {
                if content.is_some() {
                    return Err("--content may appear only once".to_owned());
                }
                content = Some(next_path(&mut arguments, "--content")?);
            }
            "--accounts" => {
                if accounts.is_some() {
                    return Err("--accounts may appear only once".to_owned());
                }
                accounts = Some(next_path(&mut arguments, "--accounts")?);
            }
            "--database-url-env" => {
                if database_env.is_some() {
                    return Err("--database-url-env may appear only once".to_owned());
                }
                database_env = Some(next_env_name(&mut arguments)?);
            }
            "--acquire-writer" => {
                if acquire_writer {
                    return Err("--acquire-writer may appear only once".to_owned());
                }
                acquire_writer = true;
            }
            "--origin" => {
                if origin.is_some() {
                    return Err("--origin may appear only once".to_owned());
                }
                origin = Some(next_unicode(&mut arguments, "--origin")?);
            }
            "--bind" => {
                if bind.is_some() {
                    return Err("--bind may appear only once".to_owned());
                }
                let value = next_unicode(&mut arguments, "--bind")?;
                bind = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|_| "--bind must be a valid socket address".to_owned())?,
                );
            }
            "--allow-insecure-http" => {
                if allow_insecure_http {
                    return Err("--allow-insecure-http may appear only once".to_owned());
                }
                allow_insecure_http = true;
            }
            "--behind-tls-ingress" => {
                if behind_tls_ingress {
                    return Err("--behind-tls-ingress may appear only once".into());
                }
                behind_tls_ingress = true;
            }
            "--operator-key-env" if operator_key_env.is_none() => {
                operator_key_env = Some(next_env_name(&mut arguments)?);
            }
            "--operator-issuer" if operator_issuer.is_none() => {
                operator_issuer = Some(next_semantic(&mut arguments, "--operator-issuer")?);
            }
            "--operator-audience" if operator_audience.is_none() => {
                operator_audience = Some(next_semantic(&mut arguments, "--operator-audience")?);
            }
            "--environment" if environment.is_none() => {
                environment = Some(
                    serde_json::from_value(serde_json::Value::String(next_semantic(
                        &mut arguments,
                        "--environment",
                    )?))
                    .map_err(|_| "--environment must be staging or production".to_owned())?,
                );
            }
            "--game" if game.is_none() => {
                game = Some(match next_unicode(&mut arguments, "--game")?.as_str() {
                    "reference" => crate::GameProfile::Reference,
                    "expedition" => crate::GameProfile::Expedition,
                    _ => return Err("--game must be reference or expedition".to_owned()),
                });
            }
            "--oidc-issuer" if oidc_issuer.is_none() => {
                oidc_issuer = Some(next_semantic(&mut arguments, "--oidc-issuer")?);
            }
            "--oidc-client-id" if oidc_client_id.is_none() => {
                oidc_client_id = Some(next_semantic(&mut arguments, "--oidc-client-id")?);
            }
            "--oidc-client-secret-env" if oidc_secret_env.is_none() => {
                oidc_secret_env = Some(next_env_name(&mut arguments)?);
            }
            "--oidc-redirect-uri" if oidc_redirect.is_none() => {
                oidc_redirect = Some(next_semantic(&mut arguments, "--oidc-redirect-uri")?);
            }
            "--operator-key-env"
            | "--operator-issuer"
            | "--operator-audience"
            | "--environment"
            | "--game" => {
                return Err("operator options may appear only once".to_owned());
            }
            _ => return Err("unknown serve option".to_owned()),
        }
    }
    let storage = match (accounts, database_env, acquire_writer) {
        (Some(accounts_path), None, false) => ParsedStorage::Local {
            accounts_path,
            content_path: content.ok_or_else(|| "--content is required".to_owned())?,
        },
        (None, Some(env_name), true) if content.is_none() => ParsedStorage::Postgres { env_name },
        (None, Some(_), true) => {
            return Err("--content is valid only for local mode".to_owned());
        }
        (None, Some(_), false) => {
            return Err("--acquire-writer is required for PostgreSQL mode".to_owned());
        }
        (Some(_), _, true) => {
            return Err("--acquire-writer is valid only for PostgreSQL mode".to_owned());
        }
        (Some(_), Some(_), _) => return Err("exactly one storage mode is required".to_owned()),
        (None, None, _) => return Err("exactly one storage mode is required".to_owned()),
    };
    if behind_tls_ingress {
        if !matches!(storage, ParsedStorage::Postgres { .. }) {
            return Err("--behind-tls-ingress requires PostgreSQL storage".into());
        }
        if allow_insecure_http {
            return Err("--behind-tls-ingress cannot combine with --allow-insecure-http".into());
        }
    } else if bind.as_ref().is_some_and(|v| !v.ip().is_loopback()) {
        return Err("--bind must be a loopback address unless --behind-tls-ingress is set".into());
    }
    let oidc_count = usize::from(oidc_issuer.is_some())
        + usize::from(oidc_client_id.is_some())
        + usize::from(oidc_secret_env.is_some())
        + usize::from(oidc_redirect.is_some());
    if oidc_count != 0 && oidc_count != 4 {
        return Err("OIDC options must be supplied all together".into());
    }
    if oidc_count != 0 && !matches!(storage, ParsedStorage::Postgres { .. }) {
        return Err("OIDC requires PostgreSQL storage".into());
    }
    let oidc = oidc_issuer
        .zip(oidc_client_id)
        .zip(oidc_secret_env)
        .zip(oidc_redirect)
        .map(
            |(((issuer, client_id), secret_env), redirect)| ParsedOidcConfig {
                issuer,
                client_id,
                client_secret_env: secret_env,
                redirect_uri: redirect,
            },
        );
    let operator_count = usize::from(operator_key_env.is_some())
        + usize::from(operator_issuer.is_some())
        + usize::from(operator_audience.is_some())
        + usize::from(environment.is_some());
    if operator_count != 0 && operator_count != 4 {
        return Err("operator options must be supplied all together".to_owned());
    }
    if behind_tls_ingress && operator_count != 0 {
        return Err("--behind-tls-ingress cannot expose operator control".to_owned());
    }
    let origin = origin.ok_or_else(|| "--origin is required".to_owned())?;
    validate_origin(&origin, allow_insecure_http)?;
    let bind = bind.unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)));
    if behind_tls_ingress && bind.ip().is_loopback() {
        return Err("--behind-tls-ingress requires a non-loopback bind".to_owned());
    }
    if !behind_tls_ingress && !bind.ip().is_loopback() {
        return Err("--bind must be a loopback address".to_owned());
    }
    Ok(Command::Serve(ServeOptions {
        bind,
        allowed_origin: origin,
        storage,
        allow_insecure_http,
        behind_tls_ingress,
        operator_key_env,
        operator_issuer,
        operator_audience,
        environment,
        game: game.unwrap_or_default(),
        oidc,
    }))
}

fn resolve_env(name: &str) -> Result<String, String> {
    resolve_env_with(name, &|name| std::env::var_os(name))
}

fn resolve_env_with(
    name: &str,
    lookup: &dyn Fn(&str) -> Option<OsString>,
) -> Result<String, String> {
    let value = lookup(name)
        .ok_or_else(|| "database URL environment variable is missing or non-Unicode".to_owned())?
        .into_string()
        .map_err(|_| "database URL environment variable is missing or non-Unicode".to_owned())?;
    if value.is_empty() {
        return Err("database URL environment variable is empty".to_owned());
    }
    Ok(value)
}

fn next_path<I>(arguments: &mut I, option: &str) -> Result<PathBuf, String>
where
    I: Iterator<Item = OsString>,
{
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    if value.is_empty() || value.to_str().is_some_and(|value| value.starts_with('-')) {
        return Err(format!("{option} requires a path value"));
    }
    Ok(PathBuf::from(value))
}

fn next_unicode<I>(arguments: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = OsString>,
{
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    let value = value
        .into_string()
        .map_err(|_| format!("{option} must be valid Unicode"))?;
    if value.is_empty() || value.starts_with('-') {
        return Err(format!("{option} requires a value"));
    }
    Ok(value)
}

fn next_semantic<I>(arguments: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = OsString>,
{
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    let value = value
        .into_string()
        .map_err(|_| format!("{option} must be valid Unicode"))?;
    if value.is_empty() {
        return Err(format!("{option} requires a value"));
    }
    Ok(value)
}

fn parse_release<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = arguments.into_iter();
    let subcommand = args
        .next()
        .ok_or_else(|| "release subcommand is required".to_owned())?
        .into_string()
        .map_err(|_| "release subcommand must be Unicode".to_owned())?;
    let mut database_env = None;
    let mut token_env = None;
    let mut key_env = None;
    let mut issuer = None;
    let mut audience = None;
    let mut environment = None;
    let mut artifact = None;
    let mut kind = None;
    let mut target = None;
    let mut expected_current = None;
    let mut request = None;
    let mut decision = None;
    let mut reason = None;
    while let Some(argument) = args.next() {
        let option = argument
            .to_str()
            .ok_or_else(|| "release options must be valid Unicode".to_owned())?;
        match option {
            "--database-url-env" if database_env.is_none() => {
                database_env = Some(next_env_name(&mut args)?)
            }
            "--operator-token-env" if token_env.is_none() => {
                token_env = Some(next_env_name(&mut args)?)
            }
            "--operator-key-env" if key_env.is_none() => key_env = Some(next_env_name(&mut args)?),
            "--operator-issuer" if issuer.is_none() => {
                issuer = Some(next_semantic(&mut args, "--operator-issuer")?)
            }
            "--operator-audience" if audience.is_none() => {
                audience = Some(next_semantic(&mut args, "--operator-audience")?)
            }
            "--environment" if environment.is_none() => {
                environment = Some(
                    serde_json::from_value(serde_json::Value::String(next_semantic(
                        &mut args,
                        "--environment",
                    )?))
                    .map_err(|_| "invalid environment".to_owned())?,
                )
            }
            "--artifact" if artifact.is_none() => {
                artifact = Some(next_path(&mut args, "--artifact")?)
            }
            "--kind" if kind.is_none() => {
                kind = Some(
                    next_semantic(&mut args, "--kind")?
                        .parse()
                        .map_err(|_| "invalid publication kind".to_owned())?,
                )
            }
            "--target" if target.is_none() => {
                target = Some(
                    next_semantic(&mut args, "--target")?
                        .parse()
                        .map_err(|_| "invalid deployment target".to_owned())?,
                )
            }
            "--expected-current" if expected_current.is_none() => {
                let value = next_semantic(&mut args, "--expected-current")?;
                expected_current = Some(if value == "none" {
                    None
                } else {
                    Some(
                        value
                            .parse()
                            .map_err(|_| "invalid expected-current digest".to_owned())?,
                    )
                });
            }
            "--request" if request.is_none() => {
                request = Some(
                    next_semantic(&mut args, "--request")?
                        .parse()
                        .map_err(|_| "invalid publication request ID".to_owned())?,
                )
            }
            "--decision" if decision.is_none() => {
                decision = Some(
                    next_semantic(&mut args, "--decision")?
                        .parse()
                        .map_err(|_| "invalid review decision".to_owned())?,
                )
            }
            "--reason" if reason.is_none() => reason = Some(next_semantic(&mut args, "--reason")?),
            "--database-url-env"
            | "--operator-token-env"
            | "--operator-key-env"
            | "--operator-issuer"
            | "--operator-audience"
            | "--environment"
            | "--artifact"
            | "--kind"
            | "--target"
            | "--expected-current"
            | "--request"
            | "--decision"
            | "--reason" => {
                return Err(format!("duplicate option {option}"));
            }
            _ => return Err("invalid release option".to_owned()),
        }
    }
    let make_auth = |reason: String| -> Result<ReleaseAuth, String> {
        Ok(ReleaseAuth {
            database_env: database_env
                .clone()
                .ok_or("--database-url-env is required")?,
            token_env: token_env
                .clone()
                .ok_or("--operator-token-env is required")?,
            key_env: key_env.clone().ok_or("--operator-key-env is required")?,
            issuer: issuer.clone().ok_or("--operator-issuer is required")?,
            audience: audience.clone().ok_or("--operator-audience is required")?,
            environment: environment.ok_or("--environment is required")?,
            reason,
        })
    };
    match subcommand.as_str() {
        "request" => {
            if request.is_some() || decision.is_some() {
                return Err("option is not valid for release request".to_owned());
            }
            Ok(Command::Release(ReleaseCommand::Request {
                auth: make_auth(reason.ok_or("--reason is required")?)?,
                artifact: artifact.ok_or_else(|| "--artifact is required".to_owned())?,
                kind: kind.ok_or_else(|| "--kind is required".to_owned())?,
                target: target.ok_or_else(|| "--target is required".to_owned())?,
                expected_current: expected_current
                    .ok_or_else(|| "--expected-current is required".to_owned())?,
            }))
        }
        "review" => {
            if artifact.is_some()
                || kind.is_some()
                || target.is_some()
                || expected_current.is_some()
            {
                return Err("option is not valid for release review".to_owned());
            }
            Ok(Command::Release(ReleaseCommand::Review {
                auth: make_auth(reason.ok_or("--reason is required")?)?,
                request: request.ok_or_else(|| "--request is required".to_owned())?,
                decision: decision.ok_or_else(|| "--decision is required".to_owned())?,
            }))
        }
        "status" => {
            if artifact.is_some()
                || kind.is_some()
                || target.is_some()
                || expected_current.is_some()
                || decision.is_some()
                || reason.is_some()
            {
                return Err("option is not valid for release status".to_owned());
            }
            Ok(Command::Release(ReleaseCommand::Status {
                auth: make_auth("view release status".to_owned())?,
                request: request.ok_or_else(|| "--request is required".to_owned())?,
            }))
        }
        "activate" => {
            if artifact.is_some()
                || kind.is_some()
                || target.is_some()
                || expected_current.is_some()
                || decision.is_some()
            {
                return Err("option is not valid for release activate".to_owned());
            }
            Ok(Command::Release(ReleaseCommand::Activate {
                auth: make_auth(reason.ok_or("--reason is required")?)?,
                request: request.ok_or_else(|| "--request is required".to_owned())?,
            }))
        }
        "recover" => {
            if artifact.is_some()
                || kind.is_some()
                || target.is_some()
                || expected_current.is_some()
                || request.is_some()
                || decision.is_some()
            {
                return Err("option is not valid for release recover".to_owned());
            }
            Ok(Command::Release(ReleaseCommand::Recover {
                auth: make_auth(reason.ok_or("--reason is required")?)?,
            }))
        }
        _ => Err("invalid release subcommand".to_owned()),
    }
}

pub(crate) fn validate_origin(origin: &str, allow_insecure_http: bool) -> Result<(), String> {
    if origin.ends_with('/') || origin.contains('@') || origin.contains('?') || origin.contains('#')
    {
        return Err("origin must be a canonical scheme://authority".to_owned());
    }
    let uri: Uri = format!("{origin}/")
        .parse()
        .map_err(|_| "origin must be a canonical scheme://authority".to_owned())?;
    let scheme = uri
        .scheme_str()
        .ok_or_else(|| "origin must include a scheme".to_owned())?;
    let authority = uri
        .authority()
        .ok_or_else(|| "origin must include an authority".to_owned())?;
    if uri.path() != "/" || uri.query().is_some() || authority.as_str().contains('@') {
        return Err("origin must be a canonical scheme://authority".to_owned());
    }
    let expected_scheme = if allow_insecure_http { "http" } else { "https" };
    if scheme != expected_scheme {
        return Err(if allow_insecure_http {
            "--allow-insecure-http requires an http origin".to_owned()
        } else {
            "default mode requires an https origin".to_owned()
        });
    }
    if allow_insecure_http && !is_loopback_host(authority.host()) {
        return Err("insecure HTTP requires a loopback origin host".to_owned());
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

pub(crate) async fn run<I>(arguments: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    let command = match parse(arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("worldwright-server: {error}");
            eprintln!("use --help for usage");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            print_help();
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("worldwright-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Status => {
            println!("worldwright-server: protocol v1/browser transport is implemented");
            println!("worldwright-server: local mode is process-local");
            println!(
                "worldwright-server: PostgreSQL durable mode uses the verified active release artifact"
            );
            println!(
                "worldwright-server: database and release workflows are implemented; status is not readiness"
            );
            println!(
                "worldwright-server: reference and expedition profiles support verified PostgreSQL state"
            );
            println!("worldwright-server: status is not a readiness probe");
            ExitCode::SUCCESS
        }
        Command::HashPassword => match hash_password() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                ExitCode::from(1)
            }
        },
        Command::Database(database) => match run_database(database).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                ExitCode::from(1)
            }
        },
        Command::Serve(options) => {
            let config = match serve_config(options) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("worldwright-server: {error}");
                    return ExitCode::from(1);
                }
            };
            match BoundServer::bind(config).await {
                Ok(server) => {
                    println!("worldwright-server listening on {}", server.local_addr());
                    println!(
                        "worldwright-server browser origin: {}",
                        server.allowed_origin()
                    );
                    if io::stdout().flush().is_err() {
                        return ExitCode::from(1);
                    }
                    // Install handlers before serving. Unix SIGTERM/SIGHUP must enter the
                    // same bounded drain path as Ctrl+C, not terminate PID 1 abruptly.
                    let shutdown = match crate::lifecycle::shutdown_signal() {
                        Ok(shutdown) => shutdown,
                        Err(error) => {
                            eprintln!("worldwright-server: shutdown signal registration failed: {error}");
                            return ExitCode::from(1);
                        }
                    };
                    match server.serve(shutdown).await {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(error) => {
                            eprintln!("worldwright-server: {error}");
                            ExitCode::from(1)
                        }
                    }
                }
                Err(error) => {
                    eprintln!("worldwright-server: {error}");
                    ExitCode::from(1)
                }
            }
        }
        Command::Release(command) => match run_release(command).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                match error {
                    ReleaseRunError::Conflict => ExitCode::from(3),
                    ReleaseRunError::Message(_) => ExitCode::from(1),
                }
            }
        },
        Command::Operator(OperatorCommand::Bootstrap {
            env_name,
            subject,
            display_name,
            kind,
            environment,
            capabilities,
            reason,
        }) => match run_operator_bootstrap(
            env_name,
            subject,
            display_name,
            kind,
            environment,
            capabilities,
            reason,
        )
        .await
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                ExitCode::from(1)
            }
        },
        Command::Backup(command) => match run_backup(command).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                ExitCode::from(1)
            }
        },
        Command::Takeover(options) => match run_takeover(options).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("worldwright-server: {error}");
                ExitCode::from(1)
            }
        },
    }
}

async fn run_backup(command: BackupCommand) -> Result<(), ServerError> {
    match command {
        BackupCommand::Verify {
            input,
            manifest,
            key_env,
        } => {
            let digest = stream_digest(&input)?;
            let signed: worldwright_operations::SignedBackupManifest =
                serde_json::from_slice(&bounded_file(&manifest, 64 * 1024)?)
                    .map_err(|_| ServerError::startup("backup manifest is invalid"))?;
            let key = decode_hex_key(
                &resolve_env(&key_env)
                    .map_err(|_| ServerError::startup("backup signing key unavailable"))?,
            )
            .map_err(|_| ServerError::startup("backup signing key invalid"))?;
            verify_backup_manifest(&signed, &key, digest)
                .map_err(|_| ServerError::startup("backup verification failed"))?;
            println!("verified");
            Ok(())
        }
        BackupCommand::Create {
            database_env,
            auth,
            service,
            output,
            key_env,
            dump,
        } => {
            validate_service(&service)?;
            let db = resolve_env(&database_env)
                .map_err(|_| ServerError::startup("database unavailable"))?;
            let store = worldwright_persistence::PostgresStore::connect(&db)
                .await
                .map_err(|_| ServerError::startup("database connection failed"))?;
            let claims = crate::release_cli::load_claims_public(&auth)
                .map_err(|_| ServerError::startup("operator token invalid"))?;
            let context = crate::release_cli::make_context_public(&auth, &claims)
                .map_err(|_| ServerError::startup("operator context invalid"))?;
            let status = store
                .operational_status(&context)
                .await
                .map_err(|_| ServerError::startup("operational status unavailable"))?;
            let job = worldwright_core::ids::RecoveryJobId::from_u128(u128::from_be_bytes(
                random_bytes::<16>()?,
            ))
            .map_err(|_| ServerError::startup("backup identity failed"))?;
            let begin_operation = random_id::<worldwright_core::ids::OperationId>()
                .map_err(|_| ServerError::startup("operation identity failed"))?;
            let success_operation = random_id::<worldwright_core::ids::OperationId>()
                .map_err(|_| ServerError::startup("operation identity failed"))?;
            let failure_operation = random_id::<worldwright_core::ids::OperationId>()
                .map_err(|_| ServerError::startup("operation identity failed"))?;
            let created_at = now_secs()?;
            let reserve = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(|_| ServerError::startup("backup output already exists"))?;
            drop(reserve);
            let mut manifest_os = output.as_os_str().to_owned();
            manifest_os.push(".manifest.json");
            let manifest_path = PathBuf::from(manifest_os);
            let mut manifest_file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&manifest_path)
            {
                Ok(file) => file,
                Err(_) => {
                    let _ = fs::remove_file(&output);
                    return Err(ServerError::startup(
                        "backup manifest output already exists",
                    ));
                }
            };
            if store
                .begin_recovery_job(
                    &context,
                    job,
                    "backup",
                    begin_operation,
                    serde_json::json!({"phase":"pg_dump"}),
                )
                .await
                .is_err()
            {
                drop(manifest_file);
                let _ = fs::remove_file(&manifest_path);
                let _ = fs::remove_file(&output);
                return Err(ServerError::startup("backup recovery job failed"));
            }
            let artifact_result = (|| {
                let tool = dump.unwrap_or_else(|| PathBuf::from("pg_dump"));
                let args = vec![
                    OsString::from("--format=custom"),
                    OsString::from("--no-owner"),
                    OsString::from("--no-privileges"),
                    OsString::from("--file"),
                    output.as_os_str().to_owned(),
                    OsString::from(format!("--dbname=service={service}")),
                ];
                run_pg_tool(&tool, &args, Duration::from_secs(600))?;
                let backup_digest = stream_digest(&output)?;
                let manifest = BackupManifest {
                    schema_version: worldwright_operations::BACKUP_MANIFEST_VERSION,
                    backup_id: job.to_string(),
                    world_id: context.world.to_string(),
                    created_at,
                    application_version: env!("CARGO_PKG_VERSION").into(),
                    active_release: status.active_release.map(|value| value.to_string()),
                    authority_incarnation: status.authority_incarnation.to_string(),
                    writer_epoch: status.writer_epoch,
                    backup_sha256: worldwright_operations::encode_hex(&backup_digest),
                    rpo_target_seconds: worldwright_operations::RPO_TARGET_SECONDS,
                    rto_target_seconds: worldwright_operations::RTO_TARGET_SECONDS,
                    audit_retention_days: worldwright_operations::AUDIT_RETENTION_DAYS,
                };
                let key = decode_hex_key(
                    &resolve_env(&key_env)
                        .map_err(|_| ServerError::startup("backup signing key unavailable"))?,
                )
                .map_err(|_| ServerError::startup("backup signing key invalid"))?;
                let signed = sign_backup_manifest(&manifest, &key)
                    .map_err(|_| ServerError::startup("backup manifest signing failed"))?;
                let manifest_bytes = serde_json::to_vec(&signed)
                    .map_err(|_| ServerError::startup("manifest serialization failed"))?;
                write_manifest_bytes(&mut manifest_file, &manifest_bytes)?;
                Ok::<_, ServerError>((backup_digest, sha256_bytes(&manifest_bytes)))
            })();
            let (backup_digest, manifest_digest) = match artifact_result {
                Ok(value) => value,
                Err(error) => {
                    drop(manifest_file);
                    let _ = fs::remove_file(&manifest_path);
                    let _ = fs::remove_file(&output);
                    let _ = store
                        .complete_recovery_job(
                            &context,
                            "backup",
                            job,
                            "failed",
                            None,
                            None,
                            None,
                            failure_operation,
                            serde_json::json!({"phase":"failed"}),
                        )
                        .await;
                    return Err(error);
                }
            };
            if store
                .complete_recovery_job(
                    &context,
                    "backup",
                    job,
                    "succeeded",
                    Some(backup_digest),
                    Some(manifest_digest),
                    None,
                    success_operation,
                    serde_json::json!({"phase":"succeeded"}),
                )
                .await
                .is_err()
            {
                return Err(ServerError::startup(
                    "backup result is durable-state uncertain",
                ));
            }
            println!("backup created: {}", output.display());
            Ok(())
        }
        BackupCommand::Restore {
            control_env,
            target_env,
            auth,
            input,
            manifest,
            key_env,
            service,
            restore,
            confirm,
        } => {
            run_backup_restore(
                control_env,
                target_env,
                auth,
                input,
                manifest,
                key_env,
                service,
                restore,
                confirm,
            )
            .await
        }
    }
}
#[allow(clippy::too_many_arguments)]
async fn run_backup_restore(
    control_env: String,
    target_env: String,
    auth: ReleaseAuth,
    input: PathBuf,
    manifest: PathBuf,
    key_env: String,
    service: String,
    restore: Option<PathBuf>,
    confirm: String,
) -> Result<(), ServerError> {
    validate_service(&service)?;
    let control_url = resolve_env(&control_env)
        .map_err(|_| ServerError::startup("control database unavailable"))?;
    let target_url = resolve_env(&target_env)
        .map_err(|_| ServerError::startup("target database unavailable"))?;
    let options = sqlx::postgres::PgConnectOptions::from_str(&target_url)
        .map_err(|_| ServerError::startup("target database URL invalid"))?;
    if options.get_database() != Some(confirm.as_str()) {
        return Err(ServerError::startup(
            "target database confirmation does not match",
        ));
    }
    let backup_digest = stream_digest(&input)?;
    let manifest_bytes = bounded_file(&manifest, 64 * 1024)?;
    let manifest_digest = sha256_bytes(&manifest_bytes);
    let signed: worldwright_operations::SignedBackupManifest =
        serde_json::from_slice(&manifest_bytes)
            .map_err(|_| ServerError::startup("backup manifest invalid"))?;
    let key = decode_hex_key(
        &resolve_env(&key_env)
            .map_err(|_| ServerError::startup("backup signing key unavailable"))?,
    )
    .map_err(|_| ServerError::startup("backup signing key invalid"))?;
    verify_backup_manifest(&signed, &key, backup_digest)
        .map_err(|_| ServerError::startup("backup verification failed"))?;
    let store = worldwright_persistence::PostgresStore::connect(&control_url)
        .await
        .map_err(|_| ServerError::startup("control database connection failed"))?;
    let claims = crate::release_cli::load_claims_public(&auth)
        .map_err(|_| ServerError::startup("operator token invalid"))?;
    let context = crate::release_cli::make_context_public(&auth, &claims)
        .map_err(|_| ServerError::startup("operator context invalid"))?;
    if signed.manifest.world_id != context.world.to_string() {
        return Err(ServerError::startup(
            "backup world does not match target scope",
        ));
    }
    let job =
        worldwright_core::ids::RecoveryJobId::from_u128(u128::from_be_bytes(random_bytes::<16>()?))
            .map_err(|_| ServerError::startup("recovery identity failed"))?;
    let begin_operation = random_id::<worldwright_core::ids::OperationId>()
        .map_err(|_| ServerError::startup("operation identity failed"))?;
    let success_operation = random_id::<worldwright_core::ids::OperationId>()
        .map_err(|_| ServerError::startup("operation identity failed"))?;
    let failure_operation = random_id::<worldwright_core::ids::OperationId>()
        .map_err(|_| ServerError::startup("operation identity failed"))?;
    let started_at = now_secs()?;
    let started = Instant::now();
    let rpo_seconds = started_at.saturating_sub(signed.manifest.created_at);
    store
        .begin_recovery_job(
            &context,
            job,
            "restore",
            begin_operation,
            serde_json::json!({
                "phase":"restore",
                "rpo_seconds": rpo_seconds,
                "rpo_target_seconds": worldwright_operations::RPO_TARGET_SECONDS
            }),
        )
        .await
        .map_err(|_| ServerError::startup("recovery job failed"))?;
    let restore_result: Result<(), ServerError> = async {
        let target_pool = sqlx::PgPool::connect_with(options)
            .await
            .map_err(|_| ServerError::startup("target database connection failed"))?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT to_regclass('worldwright_worlds')::TEXT")
                .fetch_one(&target_pool)
                .await
                .map_err(|_| ServerError::startup("target emptiness check failed"))?;
        if existing.is_some() {
            target_pool.close().await;
            return Err(ServerError::startup("target database is not empty"));
        }
        target_pool.close().await;
        let tool = restore.unwrap_or_else(|| PathBuf::from("pg_restore"));
        let args = vec![
            OsString::from("--exit-on-error"),
            OsString::from("--single-transaction"),
            OsString::from("--no-owner"),
            OsString::from("--no-privileges"),
            OsString::from(format!("--dbname=service={service}")),
            input.as_os_str().to_owned(),
        ];
        run_pg_tool(&tool, &args, Duration::from_secs(600))?;
        let target_store = worldwright_persistence::PostgresStore::connect(&target_url)
            .await
            .map_err(|_| ServerError::startup("restored target connection failed"))?;
        target_store
            .verify_migrations()
            .await
            .map_err(|_| ServerError::startup("restored migrations invalid"))?;
        crate::release_runtime::verify_restored_world(&target_store, context.world)
            .await
            .map_err(|_| ServerError::startup("restored world verification failed"))
    }
    .await;
    let rto_seconds = started.elapsed().as_secs();
    let summary = serde_json::json!({
        "phase": if restore_result.is_ok() { "verified" } else { "failed" },
        "rpo_seconds": rpo_seconds,
        "rpo_target_seconds": worldwright_operations::RPO_TARGET_SECONDS,
        "rto_seconds": rto_seconds,
        "rto_target_seconds": worldwright_operations::RTO_TARGET_SECONDS
    });
    if let Err(error) = restore_result {
        let _ = store
            .complete_recovery_job(
                &context,
                "restore",
                job,
                "failed",
                Some(backup_digest),
                Some(manifest_digest),
                None,
                failure_operation,
                summary,
            )
            .await;
        return Err(error);
    }
    if store
        .complete_recovery_job(
            &context,
            "restore",
            job,
            "succeeded",
            Some(backup_digest),
            Some(manifest_digest),
            None,
            success_operation,
            summary,
        )
        .await
        .is_err()
    {
        return Err(ServerError::startup(
            "restore result is durable-state uncertain",
        ));
    }
    println!("rpo seconds: {rpo_seconds}");
    println!(
        "rpo target seconds: {}",
        worldwright_operations::RPO_TARGET_SECONDS
    );
    println!("rto seconds: {rto_seconds}");
    println!(
        "rto target seconds: {}",
        worldwright_operations::RTO_TARGET_SECONDS
    );
    println!("restore verified; takeover not performed");
    Ok(())
}
async fn run_takeover(options: TakeoverOptions) -> Result<(), ServerError> {
    if !options.confirm {
        return Err(ServerError::Configuration(
            "--confirm-old-writers-isolated is required",
        ));
    }
    let database_url = resolve_env(&options.auth.database_env)
        .map_err(|_| ServerError::startup("database URL environment variable is unavailable"))?;
    let store = worldwright_persistence::PostgresStore::connect(&database_url)
        .await
        .map_err(|_| ServerError::startup("database connection failed"))?;
    store
        .verify_migrations()
        .await
        .map_err(|_| ServerError::startup("database migrations are not current"))?;
    let claims = crate::release_cli::load_claims_public(&options.auth)
        .map_err(|_| ServerError::startup("operator token invalid"))?;
    let context = crate::release_cli::make_context_public(&options.auth, &claims)
        .map_err(|_| ServerError::startup("operator context invalid"))?;
    let writer = random_id::<worldwright_core::ids::WriterId>()
        .map_err(|_| ServerError::startup("writer identity generation failed"))?;
    let operation = random_id::<worldwright_core::ids::OperationId>()
        .map_err(|_| ServerError::startup("operation identity generation failed"))?;
    let lease = store
        .rotate_authority(&context, writer, operation)
        .await
        .map_err(|_| ServerError::startup("authority rotation failed"))?;
    println!("world: {}", lease.world());
    println!("authority incarnation: {}", lease.incarnation());
    println!("writer epoch: {}", lease.epoch());
    println!("takeover complete");
    Ok(())
}
fn validate_service(v: &str) -> Result<(), ServerError> {
    if v.is_empty()
        || v.len() > 128
        || !v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        Err(ServerError::Configuration("pg service name invalid"))
    } else {
        Ok(())
    }
}
fn bounded_file(p: &PathBuf, max: usize) -> Result<Vec<u8>, ServerError> {
    let m = fs::metadata(p).map_err(|_| ServerError::startup("file unavailable"))?;
    if !m.is_file() || m.len() == 0 || m.len() > max as u64 {
        return Err(ServerError::startup("file bounds invalid"));
    }
    fs::read(p).map_err(|_| ServerError::startup("file unavailable"))
}
fn stream_digest(p: &PathBuf) -> Result<[u8; 32], ServerError> {
    let mut f = File::open(p).map_err(|_| ServerError::startup("backup unavailable"))?;
    let mut h = Sha256::new();
    let mut b = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut b)
            .map_err(|_| ServerError::startup("backup read failed"))?;
        if n == 0 {
            break;
        }
        h.update(&b[..n])
    }
    Ok(h.finalize().into())
}
fn write_manifest_bytes(file: &mut File, bytes: &[u8]) -> Result<(), ServerError> {
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| ServerError::startup("manifest write failed"))
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn random_bytes<const N: usize>() -> Result<[u8; N], ServerError> {
    let mut b = [0; N];
    getrandom::fill(&mut b).map_err(|_| ServerError::startup("random generation failed"))?;
    Ok(b)
}
fn now_secs() -> Result<u64, ServerError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| ServerError::startup("clock unavailable"))
}

fn run_pg_tool(
    tool: &PathBuf,
    arguments: &[OsString],
    timeout: Duration,
) -> Result<(), ServerError> {
    let mut command = ProcessCommand::new(tool);
    command.args(arguments).env_clear();
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "PGSERVICEFILE",
        "PGPASSFILE",
    ] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ServerError::startup("PostgreSQL tool failed to start"))?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| ServerError::startup("PostgreSQL tool timeout is invalid"))?;
    loop {
        match child
            .try_wait()
            .map_err(|_| ServerError::startup("PostgreSQL tool wait failed"))?
        {
            Some(status) if status.success() => return Ok(()),
            Some(_) => return Err(ServerError::startup("PostgreSQL tool failed")),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ServerError::startup("PostgreSQL tool timed out"));
            }
        }
    }
}

async fn run_operator_bootstrap(
    env_name: String,
    subject: String,
    display_name: String,
    kind: worldwright_operations::OperatorKind,
    environment: Environment,
    capabilities: Vec<Capability>,
    reason: String,
) -> Result<(), ServerError> {
    let url = resolve_env(&env_name)
        .map_err(|_| ServerError::startup("database URL environment variable is unavailable"))?;
    let store = worldwright_persistence::PostgresStore::connect(&url)
        .await
        .map_err(|_| ServerError::startup("database connection failed"))?;
    let world = worldwright_core::ids::WorldId::from_u128(1)
        .map_err(|_| ServerError::startup("world identity failed"))?;
    let correlation = random_id::<worldwright_core::ids::CorrelationId>()
        .map_err(|_| ServerError::startup("correlation identity generation failed"))?;
    let operation = random_id::<worldwright_core::ids::OperationId>()
        .map_err(|_| ServerError::startup("operation identity generation failed"))?;
    let mut grants = Vec::with_capacity(capabilities.len());
    for _ in &capabilities {
        grants.push(
            random_id::<worldwright_core::ids::GrantId>()
                .map_err(|_| ServerError::startup("grant identity generation failed"))?,
        );
    }
    let input = worldwright_persistence::BootstrapOperator {
        subject: &subject,
        display_name: &display_name,
        kind,
        environment,
        world,
        capabilities: &capabilities,
        reason: &reason,
        correlation,
    };
    store
        .bootstrap_operator(&input, operation, &grants)
        .await
        .map_err(|_| ServerError::startup("operator bootstrap failed"))?;
    println!("operator bootstrap completed");
    Ok(())
}

fn random_id<T>() -> Result<T, ()>
where
    T: RandomId,
{
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ())?;
        if let Some(value) = T::from_bytes(bytes) {
            return Ok(value);
        }
    }
    Err(())
}

trait RandomId: Sized {
    fn from_bytes(bytes: [u8; 16]) -> Option<Self>;
}
macro_rules! random_id_impl {
    ($($type:ty),+ $(,)?) => {
        $(impl RandomId for $type {
            fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
                <$type>::from_u128(u128::from_be_bytes(bytes)).ok()
            }
        })+
    };
}
random_id_impl!(
    worldwright_core::ids::CorrelationId,
    worldwright_core::ids::OperationId,
    worldwright_core::ids::GrantId,
    worldwright_core::ids::WriterId,
    worldwright_core::ids::InviteId
);

fn serve_config(options: ServeOptions) -> Result<ServerConfig, String> {
    let operator_fields = (
        options.operator_key_env,
        options.operator_issuer,
        options.operator_audience,
        options.environment,
    );
    let storage = match options.storage {
        ParsedStorage::Local {
            accounts_path,
            content_path,
        } => StorageConfig::Local {
            accounts_path,
            content_path,
        },
        ParsedStorage::Postgres { env_name } => StorageConfig::Postgres {
            database_url: resolve_env(&env_name)?,
        },
    };
    let operator = match operator_fields {
        (Some(env), Some(issuer), Some(audience), Some(environment)) => {
            Some(crate::OperatorConfig {
                signing_key: decode_hex_key(&resolve_env(&env)?)
                    .map_err(|_| "operator key environment value is invalid".to_owned())?,
                issuer,
                audience,
                environment,
            })
        }
        (None, None, None, None) => None,
        _ => return Err("operator options must be supplied all together".to_owned()),
    };
    if operator.is_some() && matches!(storage, StorageConfig::Local { .. }) {
        return Err("operator control requires PostgreSQL storage".to_owned());
    }
    let oidc = options
        .oidc
        .map(|value| {
            if !value.issuer.starts_with("https://")
                || !value.redirect_uri.starts_with("https://")
                || value.client_id.is_empty()
                || value.client_id.len() > 256
            {
                return Err("OIDC configuration is invalid".to_owned());
            }
            let client_secret = resolve_env(&value.client_secret_env)?;
            if client_secret.is_empty() || client_secret.len() > 2048 {
                return Err("OIDC client secret environment value is invalid".to_owned());
            }
            Ok(crate::OidcConfig {
                issuer: value.issuer,
                client_id: value.client_id,
                client_secret,
                redirect_uri: value.redirect_uri,
            })
        })
        .transpose()?;
    Ok(ServerConfig {
        bind: options.bind,
        allowed_origin: options.allowed_origin,
        storage,
        allow_insecure_http: options.allow_insecure_http,
        behind_tls_ingress: options.behind_tls_ingress,
        game: options.game,
        operator,
        oidc,
    })
}

async fn run_database(command: DatabaseCommand) -> Result<(), ServerError> {
    match command {
        DatabaseCommand::Migrate { env_name } => {
            let url = resolve_env(&env_name).map_err(|_| {
                ServerError::startup("database URL environment variable is unavailable")
            })?;
            let store = worldwright_persistence::PostgresStore::connect(&url)
                .await
                .map_err(|_| ServerError::startup("database connection failed"))?;
            store
                .migrate()
                .await
                .map_err(|_| ServerError::startup("database migration failed"))?;
            store
                .verify_migrations()
                .await
                .map_err(|_| ServerError::startup("database migration verification failed"))?;
        }
        DatabaseCommand::Seed {
            env_name,
            content,
            accounts,
            game,
        } => {
            let url = resolve_env(&env_name).map_err(|_| {
                ServerError::startup("database URL environment variable is unavailable")
            })?;
            let store = worldwright_persistence::PostgresStore::connect(&url)
                .await
                .map_err(|_| ServerError::startup("database connection failed"))?;
            store
                .verify_migrations()
                .await
                .map_err(|_| ServerError::startup("database migration verification failed"))?;
            let validated = worldwright_content::load_file(&content, Default::default())
                .map_err(|_| ServerError::startup("content validation failed"))?;
            let accounts = crate::auth::load_seed_accounts(&accounts)?;
            let seed = worldwright_persistence::SeedWorld {
                world: worldwright_core::ids::WorldId::from_u128(1)
                    .map_err(|_| ServerError::startup("world identity failed"))?,
                content_namespace: validated.namespace().to_owned(),
                content_schema_version: validated.schema_version(),
                game,
                entry_room_definition: validated.entry_room().as_str().to_owned(),
                accounts,
            };
            store
                .seed(&seed)
                .await
                .map_err(|_| ServerError::startup("database seed failed"))?;
        }
        DatabaseCommand::OidcInvite {
            env_name,
            username,
            issuer,
            ttl_seconds,
        } => {
            if !(worldwright_persistence::MIN_PLAYER_INVITE_TTL_SECONDS
                ..=worldwright_persistence::MAX_PLAYER_INVITE_TTL_SECONDS)
                .contains(&ttl_seconds)
                || !issuer.starts_with("https://")
            {
                return Err(ServerError::startup("OIDC invite options are invalid"));
            }
            let url = resolve_env(&env_name).map_err(|_| {
                ServerError::startup("database URL environment variable is unavailable")
            })?;
            let store = worldwright_persistence::PostgresStore::connect(&url)
                .await
                .map_err(|_| ServerError::startup("database connection failed"))?;
            store
                .verify_migrations()
                .await
                .map_err(|_| ServerError::startup("database migration verification failed"))?;
            let secret = worldwright_operations::encode_hex(&random_bytes::<32>()?);
            let invite = random_id::<worldwright_core::ids::InviteId>()
                .map_err(|_| ServerError::startup("invite identity generation failed"))?;
            let world = worldwright_core::ids::WorldId::from_u128(1)
                .map_err(|_| ServerError::startup("world identity failed"))?;
            let token_hash = worldwright_persistence::oidc_token_hash(&secret);
            store
                .issue_oidc_invite(worldwright_persistence::OidcInviteSpec {
                    invite,
                    world,
                    username: &username,
                    issuer: &issuer,
                    token_hash: &token_hash,
                    ttl_seconds,
                })
                .await
                .map_err(|_| ServerError::startup("OIDC invite issue failed"))?;
            println!("invite_token={secret}");
        }
    }
    Ok(())
}

fn print_help() {
    println!("Worldwright authenticated multiplayer transport server");
    println!("Usage: worldwright-server <--help|--version|status|hash-password>");
    println!(
        "       worldwright-server serve --content <path> --accounts <path> --origin <origin> [--game <reference|expedition>] [--bind <socket-address>] [--allow-insecure-http]"
    );
    println!(
        "       worldwright-server serve --database-url-env <ENV_NAME> --acquire-writer --origin <origin> [--game reference] [--bind <socket-address>] [--allow-insecure-http] [--operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production>]"
    );
    println!("       worldwright-server database migrate --database-url-env <ENV_NAME>");
    println!(
        "       worldwright-server database seed --database-url-env <ENV_NAME> --content <path> --accounts <path> [--game <reference|expedition>]"
    );
    println!(
        "       worldwright-server database oidc-invite --database-url-env <ENV_NAME> --username <name> --issuer <https-url> [--ttl-seconds <300-604800>]"
    );
    println!(
        "       worldwright-server release request --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --artifact <path> --kind <activate|rollback> --target <staging|production> --expected-current <none|64hex> --reason <text>"
    );
    println!(
        "       worldwright-server release review --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --request <32hex> --decision <approve|reject> --reason <text>"
    );
    println!(
        "       worldwright-server release status --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --request <32hex>"
    );
    println!(
        "       worldwright-server release activate --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --request <32hex> --reason <text>"
    );
    println!(
        "       worldwright-server release recover --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --reason <text>"
    );
    println!(
        "       worldwright-server operator bootstrap --database-url-env <DB_ENV> --subject <subject> --display-name <name> --kind <human|service|break_glass> --environment <staging|production> --capabilities <csv> --reason <text> --confirm-first-operator"
    );
    println!(
        "       worldwright-server backup <create|verify|restore> ...  (see docs/operations/BACKUP_RESTORE.md)"
    );
    println!(
        "       worldwright-server recovery takeover --database-url-env <DB_ENV> --operator-token-env <TOKEN_ENV> --operator-key-env <KEY_ENV> --operator-issuer <issuer> --operator-audience <audience> --environment <staging|production> --reason <text> --confirm-old-writers-isolated"
    );
    println!(
        "Protocol v1 browser transport with local process-local mode and PostgreSQL active-artifact mode."
    );
    println!("Default bind: 127.0.0.1:3000; only loopback binds are accepted.");
}

fn hash_password() -> Result<(), String> {
    let first = rpassword::prompt_password("Password: ")
        .map_err(|_| "password prompt failed".to_owned())?;
    let second = rpassword::prompt_password("Confirm password: ")
        .map_err(|_| "password prompt failed".to_owned())?;
    if first != second {
        return Err("passwords do not match".to_owned());
    }
    crate::auth::validate_password(&first)
        .map_err(|_| "password is empty, too long, or contains control characters".to_owned())?;
    let hash = crate::auth::make_hash(first.as_bytes())
        .map_err(|_| "password hashing failed".to_owned())?;
    println!("{hash}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_help_status_and_hash_commands() {
        assert!(matches!(parse(args(&["--help"])), Ok(Command::Help)));
        assert!(matches!(parse(args(&["status"])), Ok(Command::Status)));
        assert!(matches!(
            parse(args(&["hash-password"])),
            Ok(Command::HashPassword)
        ));
        assert!(parse(args(&[])).is_err());
    }

    #[test]
    fn serve_game_profile_is_explicit_and_bounded() {
        let parsed = parse(args(&[
            "serve",
            "--content",
            "world.json",
            "--accounts",
            "accounts.json",
            "--origin",
            "http://localhost",
            "--allow-insecure-http",
            "--game",
            "expedition",
        ]))
        .unwrap();
        assert!(matches!(
            parsed,
            Command::Serve(ServeOptions {
                game: crate::GameProfile::Expedition,
                ..
            })
        ));
        for game in ["unknown", "Expedition"] {
            assert!(
                parse(args(&[
                    "serve",
                    "--content",
                    "world.json",
                    "--accounts",
                    "accounts.json",
                    "--origin",
                    "http://localhost",
                    "--allow-insecure-http",
                    "--game",
                    game,
                ]))
                .is_err()
            );
        }
    }

    #[test]
    fn parses_serve_and_rejects_duplicates_missing_and_non_loopback() {
        let command = parse(args(&[
            "serve",
            "--content",
            "world.json",
            "--accounts",
            "accounts.json",
            "--origin",
            "http://localhost",
            "--allow-insecure-http",
        ]));
        assert!(matches!(command, Ok(Command::Serve(_))));
        assert!(
            parse(args(&[
                "serve",
                "--content",
                "a",
                "--content",
                "b",
                "--accounts",
                "c",
                "--origin",
                "https://example.com"
            ]))
            .is_err()
        );
        assert!(matches!(
            parse(args(&[
                "serve",
                "--database-url-env",
                "WORLDWRIGHT_DATABASE_URL",
                "--acquire-writer",
                "--origin",
                "https://example.com",
            ])),
            Ok(Command::Serve(ServeOptions {
                storage: ParsedStorage::Postgres { .. },
                ..
            }))
        ));
        assert!(
            parse(args(&[
                "serve",
                "--database-url-env",
                "WORLDWRIGHT_DATABASE_URL",
                "--origin",
                "https://example.com",
            ]))
            .is_err()
        );
        assert!(
            parse(args(&[
                "serve",
                "--content",
                "world.json",
                "--accounts",
                "accounts.json",
                "--database-url-env",
                "WORLDWRIGHT_DATABASE_URL",
                "--acquire-writer",
                "--origin",
                "https://example.com",
            ]))
            .is_err()
        );
        assert!(
            parse(args(&[
                "serve",
                "--accounts",
                "accounts.json",
                "--origin",
                "https://example.com",
            ]))
            .is_err()
        );
        let too_long = "a".repeat(129);
        for name in ["", "1BAD", "bad-name", too_long.as_str()] {
            assert!(
                parse(args(&[
                    "serve",
                    "--database-url-env",
                    name,
                    "--acquire-writer",
                    "--origin",
                    "https://example.com",
                ]))
                .is_err()
            );
        }
        assert!(parse(args(&["serve", "--content", "a", "--accounts", "b"])).is_err());
        assert!(
            parse(args(&[
                "serve",
                "--content",
                "a",
                "--accounts",
                "b",
                "--origin",
                "https://example.com",
                "--bind",
                "0.0.0.0:3000"
            ]))
            .is_err()
        );
    }

    #[test]
    fn enforces_origin_modes_and_canonical_shape() {
        assert!(validate_origin("https://example.com", false).is_ok());
        assert!(validate_origin("https://example.com/", false).is_err());
        assert!(validate_origin("http://localhost", false).is_err());
        assert!(validate_origin("http://localhost", true).is_ok());
        assert!(validate_origin("http://127.0.0.1:3000", true).is_ok());
        assert!(validate_origin("http://[::1]", true).is_ok());
        assert!(validate_origin("http://example.com", true).is_err());
        assert!(validate_origin("https://user@example.com", false).is_err());
    }

    #[test]
    fn defers_environment_lookup_without_mutating_process_environment() {
        assert_eq!(
            resolve_env_with("DATABASE_URL", &|name| (name == "DATABASE_URL")
                .then(|| OsString::from("postgres://secret")))
            .unwrap(),
            "postgres://secret"
        );
        assert!(resolve_env_with("DATABASE_URL", &|_| None).is_err());
        assert!(resolve_env_with("DATABASE_URL", &|_| Some(OsString::from(""))).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            assert!(
                resolve_env_with("DATABASE_URL", &|_| Some(OsString::from_vec(vec![0xff])))
                    .is_err()
            );
        }
    }

    #[test]
    fn parses_release_forms_and_rejects_invalid_options() {
        let request = [
            "release",
            "request",
            "--database-url-env",
            "DATABASE_ENV",
            "--operator-token-env",
            "TOKEN_ENV",
            "--operator-key-env",
            "KEY_ENV",
            "--operator-issuer",
            "issuer",
            "--operator-audience",
            "audience",
            "--environment",
            "staging",
            "--artifact",
            "artifact.wr",
            "--kind",
            "activate",
            "--target",
            "staging",
            "--expected-current",
            "none",
            "--reason",
            "request",
        ];
        assert!(matches!(
            parse(args(&request)),
            Ok(Command::Release(ReleaseCommand::Request { .. }))
        ));
        assert!(matches!(
            parse(args(&[
                "release",
                "review",
                "--database-url-env",
                "DATABASE_ENV",
                "--operator-token-env",
                "TOKEN_ENV",
                "--operator-key-env",
                "KEY_ENV",
                "--operator-issuer",
                "issuer",
                "--operator-audience",
                "audience",
                "--environment",
                "staging",
                "--request",
                "00000000000000000000000000000001",
                "--decision",
                "approve",
                "--reason",
                "approved",
            ])),
            Ok(Command::Release(ReleaseCommand::Review { .. }))
        ));
        assert!(matches!(
            parse(args(&[
                "release",
                "status",
                "--database-url-env",
                "DATABASE_ENV",
                "--operator-token-env",
                "TOKEN_ENV",
                "--operator-key-env",
                "KEY_ENV",
                "--operator-issuer",
                "issuer",
                "--operator-audience",
                "audience",
                "--environment",
                "staging",
                "--request",
                "00000000000000000000000000000001",
            ])),
            Ok(Command::Release(ReleaseCommand::Status { .. }))
        ));
        assert!(matches!(
            parse(args(&[
                "release",
                "activate",
                "--database-url-env",
                "DATABASE_ENV",
                "--operator-token-env",
                "TOKEN_ENV",
                "--operator-key-env",
                "KEY_ENV",
                "--operator-issuer",
                "issuer",
                "--operator-audience",
                "audience",
                "--environment",
                "staging",
                "--request",
                "00000000000000000000000000000001",
                "--reason",
                "activate",
            ])),
            Ok(Command::Release(ReleaseCommand::Activate { .. }))
        ));
        assert!(matches!(
            parse(args(&[
                "release",
                "recover",
                "--database-url-env",
                "DATABASE_ENV",
                "--operator-token-env",
                "TOKEN_ENV",
                "--operator-key-env",
                "KEY_ENV",
                "--operator-issuer",
                "issuer",
                "--operator-audience",
                "audience",
                "--environment",
                "staging",
                "--reason",
                "recover",
            ])),
            Ok(Command::Release(ReleaseCommand::Recover { .. }))
        ));
        for invalid in [
            vec!["release", "request", "--database-url-env", "DATABASE_URL"],
            vec![
                "release",
                "request",
                "--database-url-env",
                "DATABASE_URL",
                "--artifact",
                "a",
                "--kind",
                "bad",
            ],
            vec![
                "release",
                "request",
                "--database-url-env",
                "DATABASE_URL",
                "--artifact",
                "a",
                "--kind",
                "activate",
                "--target",
                "staging",
                "--expected-current",
                "none",
                "--requested-by",
                "x",
                "--requested-by",
                "y",
            ],
            vec![
                "release",
                "status",
                "--database-url-env",
                "DATABASE_URL",
                "--request",
                "bad",
            ],
            vec![
                "release",
                "status",
                "--database-url-env",
                "DATABASE_URL",
                "--request",
                "00000000000000000000000000000001",
                "--extra",
                "x",
            ],
        ] {
            assert!(parse(args(&invalid)).is_err());
        }
    }
}
