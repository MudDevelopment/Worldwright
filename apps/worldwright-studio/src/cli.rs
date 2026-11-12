use std::{
    ffi::OsString,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

pub enum Command {
    Help,
    Version,
    Status,
    Serve(ServeOptions),
}
pub struct ServeOptions {
    pub workspace: PathBuf,
    pub content: Option<PathBuf>,
    pub bind: SocketAddr,
    pub preview_command: Option<PathBuf>,
    pub release_endpoint: Option<String>,
    pub operator_token_env: Option<String>,
    pub operator_token_file: Option<PathBuf>,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.len() > 32 {
        return Err("too many arguments".into());
    }
    if args.len() == 1 {
        match args[0].to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            Some("status") => return Ok(Command::Status),
            _ => {}
        }
    }
    if args.first().and_then(|v| v.to_str()) != Some("serve") {
        return Err("use --help".into());
    }
    let mut workspace = None;
    let mut content = None;
    let mut bind = SocketAddr::from(([127, 0, 0, 1], 0));
    let mut bind_seen = false;
    let mut preview = None;
    let mut endpoint = None;
    let mut token = None;
    let mut token_file = None;
    let mut i = 1;
    while i < args.len() {
        let key = args[i].to_str().ok_or("option must be UTF-8")?;
        if i + 1 >= args.len() {
            return Err(format!("{key} requires a value"));
        }
        match key {
            "--workspace" => {
                if workspace.is_some() {
                    return Err("duplicate --workspace".into());
                }
                workspace = Some(PathBuf::from(value(&args[i + 1], key)?));
            }
            "--content" => {
                if content.is_some() {
                    return Err("duplicate --content".into());
                }
                content = Some(PathBuf::from(value(&args[i + 1], key)?));
            }
            "--bind" => {
                if bind_seen {
                    return Err("duplicate --bind".into());
                }
                bind_seen = true;
                let s = value(&args[i + 1], key)?;
                let addr: SocketAddr = s.parse().map_err(|_| "bind is invalid")?;
                if !matches!(addr.ip(),IpAddr::V4(v) if v.octets()==[127,0,0,1]) {
                    return Err("bind must be exactly 127.0.0.1".into());
                }
                bind = addr;
            }
            "--preview-command" => {
                if preview.is_some() {
                    return Err("duplicate --preview-command".into());
                }
                preview = Some(PathBuf::from(value(&args[i + 1], key)?));
            }
            "--release-endpoint" => {
                if endpoint.is_some() {
                    return Err("duplicate --release-endpoint".into());
                }
                let v = value(&args[i + 1], key)?;
                validate_endpoint(&v)?;
                endpoint = Some(v);
            }
            "--operator-token-env" => {
                if token.is_some() {
                    return Err("duplicate --operator-token-env".into());
                }
                let v = value(&args[i + 1], key)?;
                validate_env(&v)?;
                token = Some(v);
            }
            "--operator-token-file" => {
                if token_file.is_some() {
                    return Err("duplicate --operator-token-file".into());
                }
                let path = PathBuf::from(value(&args[i + 1], key)?);
                if !path.is_absolute() {
                    return Err("operator token file must be absolute".into());
                }
                token_file = Some(path);
            }
            _ => return Err("unknown or cross-command option".into()),
        }
        i += 2
    }
    if (token.is_some() && token_file.is_some())
        || endpoint.is_some() != (token.is_some() || token_file.is_some())
    {
        return Err("release endpoint requires exactly one token env or token file".into());
    }
    Ok(Command::Serve(ServeOptions {
        workspace: workspace.ok_or("--workspace is required")?,
        content,
        bind,
        preview_command: preview,
        release_endpoint: endpoint,
        operator_token_env: token,
        operator_token_file: token_file,
    }))
}
fn value(v: &OsString, k: &str) -> Result<String, String> {
    let s = v.to_str().ok_or(format!("{k} must be UTF-8"))?;
    if s.is_empty() {
        return Err(format!("{k} requires a value"));
    }
    Ok(s.into())
}
fn validate_env(v: &str) -> Result<(), String> {
    if v.is_empty()
        || v.len() > 128
        || !v.as_bytes()[0].is_ascii_alphabetic() && v.as_bytes()[0] != b'_'
        || !v
            .bytes()
            .skip(1)
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        Err("environment name invalid".into())
    } else {
        Ok(())
    }
}
fn validate_endpoint(v: &str) -> Result<(), String> {
    let raw = v
        .strip_prefix("http://")
        .ok_or("release endpoint must be loopback HTTP")?;
    if raw.contains(['@', '/', '?', '#']) {
        return Err("release endpoint invalid".into());
    }
    let (host, port) = raw.rsplit_once(':').ok_or("release endpoint invalid")?;
    if host != "127.0.0.1" {
        return Err("release endpoint must be 127.0.0.1".into());
    }
    let p: u16 = port.parse().map_err(|_| "release endpoint invalid")?;
    if p == 0 {
        Err("release endpoint port must be nonzero".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_full_serve_and_rejects_duplicates() {
        let command = parse(args(&[
            "serve",
            "--workspace",
            "drafts",
            "--content",
            "world.json",
            "--bind",
            "127.0.0.1:3010",
            "--preview-command",
            "preview",
            "--release-endpoint",
            "http://127.0.0.1:3020",
            "--operator-token-env",
            "WORLDWRIGHT_TOKEN",
        ]));
        assert!(
            matches!(command, Ok(Command::Serve(ServeOptions { bind, .. })) if bind.port() == 3010)
        );
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--bind",
                "127.0.0.1:1",
                "--bind",
                "127.0.0.1:2"
            ]))
            .is_err()
        );
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--release-endpoint",
                "http://127.0.0.1:2"
            ]))
            .is_err()
        );
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--operator-token-env",
                "TOKEN"
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_bind_endpoint_environment_and_arguments() {
        for bind in ["0.0.0.0:1", "[::1]:1"] {
            assert!(parse(args(&["serve", "--workspace", "x", "--bind", bind])).is_err());
        }
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--bind",
                "127.0.0.1:0"
            ]))
            .is_ok()
        );
        for endpoint in [
            "https://127.0.0.1:1",
            "http://localhost:1",
            "http://127.0.0.1:0",
            "http://127.0.0.1:1/path",
            "http://user@127.0.0.1:1",
        ] {
            assert!(
                parse(args(&[
                    "serve",
                    "--workspace",
                    "x",
                    "--release-endpoint",
                    endpoint,
                    "--operator-token-env",
                    "TOKEN"
                ]))
                .is_err()
            );
        }
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--operator-token-env",
                "1BAD"
            ]))
            .is_err()
        );
        assert!(parse(args(&["serve", "--workspace", "x", "--unknown", "x"])).is_err());
        assert!(parse(args(&["serve", "--workspace"])).is_err());
        assert!(parse(args(&["status", "extra"])).is_err());
    }

    #[test]
    fn rejects_non_utf8_and_oversized_arguments() {
        let mut args = vec![
            OsString::from("serve"),
            OsString::from("--workspace"),
            OsString::from("x"),
        ];
        args.extend((0..30).map(|i| OsString::from(format!("--x{i}"))));
        assert!(parse(args).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            assert!(
                parse(vec![
                    OsString::from("serve"),
                    OsString::from("--workspace"),
                    OsString::from_vec(vec![0xff]),
                ])
                .is_err()
            );
        }
    }

    #[test]
    fn defaults_loopback_ephemeral_bind_and_pair_options() {
        let Ok(Command::Serve(options)) = parse(args(&["serve", "--workspace", "x"])) else {
            panic!("serve parse failed");
        };
        assert_eq!(options.bind.ip(), IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
        assert_eq!(options.bind.port(), 0);
        assert!(
            parse(args(&[
                "serve",
                "--workspace",
                "x",
                "--release-endpoint",
                "http://127.0.0.1:1"
            ]))
            .is_err()
        );
    }
}
