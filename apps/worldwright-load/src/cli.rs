use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Profile {
    Beta250,
    Smoke {
        clients: usize,
        commands: usize,
        period_ms: u64,
    },
}

impl Profile {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Beta250 => "beta-250-v1",
            Self::Smoke { .. } => "smoke-v1",
        }
    }

    pub(crate) fn dimensions(self) -> (usize, usize, u64) {
        match self {
            Self::Beta250 => (250, 30, 2_000),
            Self::Smoke {
                clients,
                commands,
                period_ms,
            } => (clients, commands, period_ms),
        }
    }
}

pub(crate) struct GenerateOptions {
    pub seed_output: PathBuf,
    pub credentials_output: PathBuf,
    pub password_env: String,
}

pub(crate) struct RunOptions {
    pub base_url: String,
    pub origin: String,
    pub credentials: PathBuf,
    pub output: PathBuf,
    pub server_revision: String,
    pub artifact_digest: String,
    pub environment_label: String,
    pub profile: Profile,
}

pub(crate) enum Command {
    Help,
    Version,
    Generate(GenerateOptions),
    Run(RunOptions),
}

pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let values = args.into_iter().collect::<Vec<_>>();
    if values.len() > 32 {
        return Err("too many arguments".into());
    }
    if values.len() == 1 && matches!(word(&values[0])?, "--help" | "-h") {
        return Ok(Command::Help);
    }
    if values.len() == 1 && matches!(word(&values[0])?, "--version" | "-V") {
        return Ok(Command::Version);
    }
    match values.first().map(word).transpose()? {
        Some("generate") => parse_generate(&values[1..]),
        Some("run") => parse_run(&values[1..], false),
        Some("__smoke") => parse_run(&values[1..], true),
        _ => Err("expected generate or run".into()),
    }
}

fn parse_generate(values: &[OsString]) -> Result<Command, String> {
    let mut seed = None;
    let mut credentials = None;
    let mut password_env = None;
    parse_pairs(values, |option, value| match option {
        "--seed-output" if seed.is_none() => assign(&mut seed, PathBuf::from(value)),
        "--credentials-output" if credentials.is_none() => {
            assign(&mut credentials, PathBuf::from(value))
        }
        "--password-env" if password_env.is_none() => assign(&mut password_env, value.to_owned()),
        "--seed-output" | "--credentials-output" | "--password-env" => {
            Err(format!("duplicate option {option}"))
        }
        _ => Err(format!("unknown option {option}")),
    })?;
    Ok(Command::Generate(GenerateOptions {
        seed_output: seed.ok_or("missing --seed-output")?,
        credentials_output: credentials.ok_or("missing --credentials-output")?,
        password_env: password_env.ok_or("missing --password-env")?,
    }))
}

fn parse_run(values: &[OsString], smoke: bool) -> Result<Command, String> {
    let mut base_url = None;
    let mut origin = None;
    let mut credentials = None;
    let mut output = None;
    let mut revision = None;
    let mut digest = None;
    let mut label = None;
    let mut clients = None;
    let mut commands = None;
    let mut period = None;
    parse_pairs(values, |option, value| match option {
        "--base-url" if base_url.is_none() => assign(&mut base_url, value.to_owned()),
        "--origin" if origin.is_none() => assign(&mut origin, value.to_owned()),
        "--credentials" if credentials.is_none() => assign(&mut credentials, PathBuf::from(value)),
        "--output" if output.is_none() => assign(&mut output, PathBuf::from(value)),
        "--server-revision" if revision.is_none() => assign(&mut revision, value.to_owned()),
        "--artifact-digest" if digest.is_none() => assign(&mut digest, value.to_owned()),
        "--environment-label" if label.is_none() => assign(&mut label, value.to_owned()),
        "--clients" if smoke && clients.is_none() => assign(&mut clients, integer(value, option)?),
        "--commands" if smoke && commands.is_none() => {
            assign(&mut commands, integer(value, option)?)
        }
        "--period-ms" if smoke && period.is_none() => assign(&mut period, integer(value, option)?),
        "--base-url"
        | "--origin"
        | "--credentials"
        | "--output"
        | "--server-revision"
        | "--artifact-digest"
        | "--environment-label"
        | "--clients"
        | "--commands"
        | "--period-ms" => Err(format!("duplicate or forbidden option {option}")),
        _ => Err(format!("unknown option {option}")),
    })?;
    let profile = if smoke {
        let clients = clients.ok_or("missing --clients")?;
        let commands = commands.ok_or("missing --commands")?;
        let period_ms = period.ok_or("missing --period-ms")? as u64;
        if !(1..=8).contains(&clients)
            || !(1..=10).contains(&commands)
            || !(10..=1_000).contains(&period_ms)
        {
            return Err("smoke dimensions are out of bounds".into());
        }
        Profile::Smoke {
            clients,
            commands,
            period_ms,
        }
    } else {
        Profile::Beta250
    };
    Ok(Command::Run(RunOptions {
        base_url: base_url.ok_or("missing --base-url")?,
        origin: origin.ok_or("missing --origin")?,
        credentials: credentials.ok_or("missing --credentials")?,
        output: output.ok_or("missing --output")?,
        server_revision: revision.ok_or("missing --server-revision")?,
        artifact_digest: digest.ok_or("missing --artifact-digest")?,
        environment_label: label.ok_or("missing --environment-label")?,
        profile,
    }))
}

fn parse_pairs(
    values: &[OsString],
    mut accept: impl FnMut(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    if values.len() % 2 != 0 {
        return Err("every option requires a value".into());
    }
    for pair in values.chunks_exact(2) {
        let option = word(&pair[0])?;
        let value = word(&pair[1])?;
        if !option.starts_with("--") || value.is_empty() {
            return Err("invalid option or empty value".into());
        }
        accept(option, value)?;
    }
    Ok(())
}

fn word(value: &OsString) -> Result<&str, String> {
    value
        .to_str()
        .ok_or_else(|| "arguments must be Unicode".into())
}

fn integer(value: &str, option: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{option} must be an integer"))
}

fn assign<T>(slot: &mut Option<T>, value: T) -> Result<(), String> {
    *slot = Some(value);
    Ok(())
}

pub(crate) fn print_help() {
    println!("Worldwright bounded network/PostgreSQL load tool");
    println!(
        "Usage: worldwright-load generate --seed-output <new.json> --credentials-output <new.json> --password-env <ENV>"
    );
    println!(
        "       worldwright-load run --base-url <https-origin> --origin <same-origin> --credentials <path> --output <new.json> --server-revision <revision> --artifact-digest <64lowerhex> --environment-label <label>"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parser_is_strict_and_beta_is_not_tunable() {
        assert!(matches!(
            parse(args(&[
                "generate",
                "--seed-output",
                "seed.json",
                "--credentials-output",
                "credentials.json",
                "--password-env",
                "LOAD_PASSWORD"
            ])),
            Ok(Command::Generate(_))
        ));
        let run = [
            "run",
            "--base-url",
            "https://world.example",
            "--origin",
            "https://world.example",
            "--credentials",
            "credentials.json",
            "--output",
            "report.json",
            "--server-revision",
            "abc",
            "--artifact-digest",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--environment-label",
            "test",
        ];
        assert!(matches!(parse(args(&run)), Ok(Command::Run(_))));
        let mut tuned = run.to_vec();
        tuned.extend(["--clients", "2"]);
        assert!(parse(args(&tuned)).is_err());
        assert!(parse(args(&["generate", "--password", "secret"])).is_err());
    }
}
