use crate::{BoundServer, ServerConfig, ServerError, StorageConfig};
use axum::http::Uri;
use std::{
    ffi::OsString,
    io::{self, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
};

pub(crate) enum Command {
    Help,
    Version,
    Status,
    HashPassword,
    Serve(ServerConfig),
    Database(DatabaseCommand),
}

pub(crate) enum DatabaseCommand {
    Migrate {
        env_name: String,
    },
    Seed {
        env_name: String,
        content: PathBuf,
        accounts: PathBuf,
    },
}

pub(crate) fn parse<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
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
        Some(_) => Err("unknown command or extra argument".to_owned()),
        None => Err("command must be valid Unicode".to_owned()),
    }
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
            _ => return Err("invalid database option".to_owned()),
        }
    }
    let env_name = env_name.ok_or_else(|| "--database-url-env is required".to_owned())?;
    match subcommand {
        "migrate" if content.is_none() && accounts.is_none() => {
            Ok(Command::Database(DatabaseCommand::Migrate { env_name }))
        }
        "seed" => Ok(Command::Database(DatabaseCommand::Seed {
            env_name,
            content: content.ok_or_else(|| "--content is required".to_owned())?,
            accounts: accounts.ok_or_else(|| "--accounts is required".to_owned())?,
        })),
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
            _ => return Err("unknown serve option".to_owned()),
        }
    }
    let content = content.ok_or_else(|| "--content is required".to_owned())?;
    let storage = match (accounts, database_env, acquire_writer) {
        (Some(accounts_path), None, false) => StorageConfig::Local { accounts_path },
        (None, Some(env_name), true) => StorageConfig::Postgres {
            database_url: resolve_env(&env_name)?,
        },
        (None, Some(_), false) => {
            return Err("--acquire-writer is required for PostgreSQL mode".to_owned());
        }
        (Some(_), _, true) => {
            return Err("--acquire-writer is valid only for PostgreSQL mode".to_owned());
        }
        (Some(_), Some(_), _) => return Err("exactly one storage mode is required".to_owned()),
        (None, None, _) => return Err("exactly one storage mode is required".to_owned()),
    };
    let origin = origin.ok_or_else(|| "--origin is required".to_owned())?;
    validate_origin(&origin, allow_insecure_http)?;
    let bind = bind.unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)));
    if !bind.ip().is_loopback() {
        return Err("--bind must be a loopback address".to_owned());
    }
    Ok(Command::Serve(ServerConfig {
        bind,
        allowed_origin: origin,
        content_path: content,
        storage,
        allow_insecure_http,
    }))
}

fn resolve_env(name: &str) -> Result<String, String> {
    let value = std::env::var(name)
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
            println!("worldwright-server: sessions and world state are process-local");
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
        Command::Serve(config) => match BoundServer::bind(config).await {
            Ok(server) => {
                println!("worldwright-server listening on {}", server.local_addr());
                println!(
                    "worldwright-server browser origin: {}",
                    server.allowed_origin()
                );
                if io::stdout().flush().is_err() {
                    return ExitCode::from(1);
                }
                let shutdown = async {
                    let _ = tokio::signal::ctrl_c().await;
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
        },
    }
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
                entry_room_definition: validated.entry_room().as_str().to_owned(),
                accounts,
            };
            store
                .seed(&seed)
                .await
                .map_err(|_| ServerError::startup("database seed failed"))?;
        }
    }
    Ok(())
}

fn print_help() {
    println!("Worldwright authenticated multiplayer transport server");
    println!("Usage: worldwright-server <--help|--version|status|hash-password>");
    println!(
        "       worldwright-server serve --content <path> --accounts <path> --origin <origin> [--bind <socket-address>] [--allow-insecure-http]"
    );
    println!("Protocol v1 browser transport with process-local sessions and world state.");
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
}
