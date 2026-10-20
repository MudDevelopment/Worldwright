use crate::{BoundServer, ServerConfig};
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
        Some("serve") => parse_serve(arguments),
        Some(_) => Err("unknown command or extra argument".to_owned()),
        None => Err("command must be valid Unicode".to_owned()),
    }
}

fn parse_serve<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut arguments = arguments.into_iter();
    let mut content = None;
    let mut accounts = None;
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
    let accounts = accounts.ok_or_else(|| "--accounts is required".to_owned())?;
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
        accounts_path: accounts,
        allow_insecure_http,
    }))
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
