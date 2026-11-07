use std::{ffi::OsString, path::PathBuf};
use worldwright_release::{ReleaseId, Sha256Digest};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    ContentHelp,
    PlayHelp,
    Play(PathBuf),
    Version,
    Status,
    Compatibility {
        json: bool,
    },
    Benchmark {
        clients: usize,
        commands_per_client: usize,
        json: bool,
    },
    ProjectNew {
        path: PathBuf,
        name: String,
        template: String,
        dry_run: bool,
    },
    Validate(PathBuf),
    ContentMigrate {
        input: PathBuf,
        output: PathBuf,
        backup: Option<PathBuf>,
        dry_run: bool,
    },
    ContentPackage {
        content: PathBuf,
        output: PathBuf,
        release_id: ReleaseId,
        source_revision: String,
        builder_identity: String,
    },
    ContentInspect {
        artifact: PathBuf,
        json: bool,
    },
    PreviewHelp,
    Preview {
        artifact: PathBuf,
        max_seconds: u64,
    },
    PreviewWorker {
        artifact: PathBuf,
        expected_digest: Sha256Digest,
        slot: PathBuf,
        token: String,
        max_seconds: u64,
    },
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut iterator = args.into_iter();
    let mut values = Vec::with_capacity(16);
    for _ in 0..16 {
        if let Some(value) = iterator.next() {
            values.push(value);
        } else {
            break;
        }
    }
    if iterator.next().is_some() {
        return Err("too many arguments".into());
    }
    parse_values(&values)
}

fn word(values: &[OsString], index: usize) -> Result<&str, String> {
    values
        .get(index)
        .ok_or_else(|| "missing argument".to_string())?
        .to_str()
        .ok_or_else(|| "command words and option values must be valid UTF-8".to_string())
}

fn is_word(values: &[OsString], index: usize, expected: &str) -> bool {
    values.get(index).and_then(|value| value.to_str()) == Some(expected)
}

fn parse_values(values: &[OsString]) -> Result<Command, String> {
    match values.len() {
        1 if is_word(values, 0, "--help") || is_word(values, 0, "-h") => Ok(Command::Help),
        1 if is_word(values, 0, "--version") || is_word(values, 0, "-V") => {
            Ok(Command::Version)
        }
        1 if is_word(values, 0, "status") => Ok(Command::Status),
        _ if is_word(values, 0, "content") => parse_content(values),
        _ if is_word(values, 0, "compatibility") => {
            if values.len() == 1 {
                Ok(Command::Compatibility { json: false })
            } else if values.len() == 2 && is_word(values, 1, "--json") {
                Ok(Command::Compatibility { json: true })
            } else {
                Err("expected compatibility [--json]".into())
            }
        }
        _ if is_word(values, 0, "benchmark") => parse_benchmark(values),
        _ if is_word(values, 0, "project") => parse_project(values),
        _ if is_word(values, 0, "play") => parse_play(values),
        _ if is_word(values, 0, "preview") => parse_preview(values),
        _ if is_word(values, 0, "__preview-worker") => parse_preview_worker(values),
        _ => Err(
            "expected --help, --version, status, compatibility, benchmark, project, content, play, or preview"
                .into(),
        ),
    }
}

fn parse_content(values: &[OsString]) -> Result<Command, String> {
    if values.len() == 2 && (is_word(values, 1, "--help") || is_word(values, 1, "-h")) {
        return Ok(Command::ContentHelp);
    }
    let subcommand = word(values, 1)?;
    match subcommand {
        "validate" => {
            if values.len() == 3 && (is_word(values, 2, "--help") || is_word(values, 2, "-h")) {
                return Ok(Command::ContentHelp);
            }
            if values.len() == 3 && !values[2].as_os_str().is_empty() {
                return Ok(Command::Validate(values[2].clone().into()));
            }
            Err("expected content validate <path>".into())
        }
        "package"
            if values.len() == 3 && (is_word(values, 2, "--help") || is_word(values, 2, "-h")) =>
        {
            Ok(Command::ContentHelp)
        }
        "package" => parse_package(values),
        "inspect"
            if values.len() == 3 && (is_word(values, 2, "--help") || is_word(values, 2, "-h")) =>
        {
            Ok(Command::ContentHelp)
        }
        "inspect" => parse_inspect(values),
        "migrate" => parse_migrate(values),
        _ => Err("expected content validate, package, inspect, or migrate".into()),
    }
}

fn parse_migrate(values: &[OsString]) -> Result<Command, String> {
    if values.len() < 5 || values[2].as_os_str().is_empty() {
        return Err(
            "expected content migrate <input> --output <path> [--backup <path>] [--dry-run]".into(),
        );
    }
    let input = values[2].clone().into();
    let mut output = None;
    let mut backup = None;
    let mut dry_run = false;
    let mut index = 3;
    while index < values.len() {
        match word(values, index)? {
            "--output" if output.is_none() => {
                output = Some(
                    values
                        .get(index + 1)
                        .ok_or("missing --output value")?
                        .clone()
                        .into(),
                );
                index += 2;
            }
            "--dry-run" if !dry_run => {
                dry_run = true;
                index += 1;
            }
            "--backup" if backup.is_none() => {
                backup = Some(
                    values
                        .get(index + 1)
                        .ok_or("missing --backup value")?
                        .clone()
                        .into(),
                );
                index += 2;
            }
            option => return Err(format!("invalid or duplicate migrate option {option}")),
        }
    }
    Ok(Command::ContentMigrate {
        input,
        output: output.ok_or("missing --output")?,
        backup,
        dry_run,
    })
}

fn parse_benchmark(values: &[OsString]) -> Result<Command, String> {
    let mut clients = 100usize;
    let mut commands_per_client = 10usize;
    let mut json = false;
    let mut index = 1;
    while index < values.len() {
        match word(values, index)? {
            "--clients" => {
                clients = word(values, index + 1)?
                    .parse()
                    .map_err(|_| "invalid --clients".to_owned())?;
                index += 2;
            }
            "--commands-per-client" => {
                commands_per_client = word(values, index + 1)?
                    .parse()
                    .map_err(|_| "invalid --commands-per-client".to_owned())?;
                index += 2;
            }
            "--json" if !json => {
                json = true;
                index += 1;
            }
            option => return Err(format!("invalid benchmark option {option}")),
        }
    }
    Ok(Command::Benchmark {
        clients,
        commands_per_client,
        json,
    })
}

fn parse_project(values: &[OsString]) -> Result<Command, String> {
    if values.len() < 5 || !is_word(values, 1, "new") {
        return Err(
            "expected project new <path> --name <package> [--template minimal|expedition] [--dry-run]"
                .into(),
        );
    }
    let path = values[2].clone().into();
    let mut name = None;
    let mut template = "minimal".to_owned();
    let mut template_seen = false;
    let mut dry_run = false;
    let mut index = 3;
    while index < values.len() {
        match word(values, index)? {
            "--name" if name.is_none() => {
                name = Some(word(values, index + 1)?.to_owned());
                index += 2;
            }
            "--template" if !template_seen => {
                template = word(values, index + 1)?.to_owned();
                template_seen = true;
                index += 2;
            }
            "--dry-run" if !dry_run => {
                dry_run = true;
                index += 1;
            }
            option => return Err(format!("invalid or duplicate project option {option}")),
        }
    }
    if !matches!(template.as_str(), "minimal" | "expedition") {
        return Err("template must be minimal or expedition".into());
    }
    Ok(Command::ProjectNew {
        path,
        name: name.ok_or("missing --name")?,
        template,
        dry_run,
    })
}

fn parse_play(values: &[OsString]) -> Result<Command, String> {
    if values.len() == 2 && (is_word(values, 1, "--help") || is_word(values, 1, "-h")) {
        return Ok(Command::PlayHelp);
    }
    if values.len() == 2 && !values[1].as_os_str().is_empty() {
        return Ok(Command::Play(values[1].clone().into()));
    }
    Err("expected play <path>".into())
}

fn parse_package(values: &[OsString]) -> Result<Command, String> {
    if values.len() < 3 || values[2].as_os_str().is_empty() {
        return Err("expected content package <content-path> and package options".into());
    }
    let content = values[2].clone().into();
    let mut output = None;
    let mut release_id = None;
    let mut source_revision = None;
    let mut builder_identity = None;
    let mut index = 3;
    while index < values.len() {
        let option = word(values, index)?;
        if values.get(index + 1).is_none() {
            return Err(format!("missing value for {option}"));
        }
        match option {
            "--output" if output.is_none() && !values[index + 1].as_os_str().is_empty() => {
                output = Some(values[index + 1].clone().into())
            }
            "--release-id" if release_id.is_none() => {
                release_id = Some(
                    word(values, index + 1)?
                        .parse()
                        .map_err(|_| "invalid release ID".to_string())?,
                )
            }
            "--source-revision" if source_revision.is_none() => {
                source_revision = Some(word(values, index + 1)?.to_owned())
            }
            "--builder" if builder_identity.is_none() => {
                builder_identity = Some(word(values, index + 1)?.to_owned())
            }
            "--output" | "--release-id" | "--source-revision" | "--builder" => {
                return Err(format!("duplicate option {option}"));
            }
            _ => return Err(format!("unknown package option {option}")),
        }
        index += 2;
    }
    Ok(Command::ContentPackage {
        content,
        output: output.ok_or("missing --output".to_string())?,
        release_id: release_id.ok_or("missing --release-id".to_string())?,
        source_revision: source_revision.ok_or("missing --source-revision".to_string())?,
        builder_identity: builder_identity.ok_or("missing --builder".to_string())?,
    })
}

fn parse_inspect(values: &[OsString]) -> Result<Command, String> {
    if values.len() < 3 || values[2].as_os_str().is_empty() {
        return Err("expected content inspect <artifact-path>".into());
    }
    let artifact = values[2].clone().into();
    let mut json = false;
    for index in 3..values.len() {
        let option = word(values, index)?;
        if option == "--json" && !json {
            json = true;
        } else if option == "--json" {
            return Err("duplicate option --json".into());
        } else {
            return Err(format!("unknown inspect option {option}"));
        }
    }
    Ok(Command::ContentInspect { artifact, json })
}

fn parse_preview(values: &[OsString]) -> Result<Command, String> {
    if values.len() == 2 && (is_word(values, 1, "--help") || is_word(values, 1, "-h")) {
        return Ok(Command::PreviewHelp);
    }
    if values.len() < 2 || values[1].as_os_str().is_empty() {
        return Err("expected preview <artifact-path>".into());
    }
    let artifact = values[1].clone().into();
    let mut max_seconds = 900;
    let mut seen = false;
    let mut index = 2;
    while index < values.len() {
        let option = word(values, index)?;
        if option != "--max-seconds" {
            return Err(format!("unknown preview option {option}"));
        }
        if seen {
            return Err("duplicate option --max-seconds".into());
        }
        if values.get(index + 1).is_none() {
            return Err("missing value for --max-seconds".into());
        }
        let seconds = word(values, index + 1)?
            .parse::<u64>()
            .map_err(|_| "max-seconds must be 1..=3600".to_string())?;
        if !(1..=3600).contains(&seconds) {
            return Err("max-seconds must be 1..=3600".into());
        }
        max_seconds = seconds;
        seen = true;
        index += 2;
    }
    Ok(Command::Preview {
        artifact,
        max_seconds,
    })
}

fn parse_preview_worker(values: &[OsString]) -> Result<Command, String> {
    if values.len() != 6 {
        return Err(
            "preview worker requires artifact, digest, slot, token, and max-seconds".into(),
        );
    }
    let artifact: PathBuf = values[1].clone().into();
    if !artifact.is_absolute() {
        return Err("preview worker artifact path must be absolute".into());
    }
    let expected_digest = word(values, 2)?
        .parse()
        .map_err(|_| "invalid expected artifact digest".to_string())?;
    let slot: PathBuf = values[3].clone().into();
    if !slot.is_absolute() {
        return Err("preview worker slot path must be absolute".into());
    }
    let token = word(values, 4)?.to_owned();
    if token.len() != 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("preview worker token must be 32 lowercase hexadecimal characters".into());
    }
    let max_seconds = word(values, 5)?
        .parse::<u64>()
        .map_err(|_| "preview worker max-seconds must be 1..=3600".to_string())?;
    if !(1..=3600).contains(&max_seconds) {
        return Err("preview worker max-seconds must be 1..=3600".into());
    }
    Ok(Command::PreviewWorker {
        artifact,
        expected_digest,
        slot,
        token,
        max_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn package_options_are_order_independent() {
        let command = parse_args(&[
            "content",
            "package",
            "world.json",
            "--builder",
            "builder",
            "--source-revision",
            "revision",
            "--release-id",
            "release-1",
            "--output",
            "world.wr",
        ])
        .unwrap();
        assert!(matches!(
            command,
            Command::ContentPackage {
                release_id,
                source_revision,
                builder_identity,
                ..
            } if release_id.as_str() == "release-1"
                && source_revision == "revision"
                && builder_identity == "builder"
        ));
    }

    #[test]
    fn package_missing_duplicate_unknown_and_extra_arguments_are_usage_errors() {
        for args in [
            vec!["content", "package", ""],
            vec!["content", "package", "world.json"],
            vec!["content", "package", "world.json", "--output"],
            vec![
                "content",
                "package",
                "world.json",
                "--output",
                "out",
                "--output",
                "other",
            ],
            vec![
                "content",
                "package",
                "world.json",
                "--output",
                "out",
                "--release-id",
                "release",
                "--source-revision",
                "revision",
                "--builder",
                "builder",
                "--unknown",
                "value",
            ],
            vec![
                "content",
                "package",
                "world.json",
                "--output",
                "out",
                "--output",
                "other",
                "--release-id",
                "release",
                "--release-id",
                "other",
                "--source-revision",
                "revision",
                "--source-revision",
                "other",
                "--builder",
                "builder",
                "--builder",
                "other",
            ],
            vec!["content", "inspect", "artifact", "--json", "--json"],
            vec!["content", "inspect", "artifact", "--extra"],
            vec!["preview", "artifact", "--max-seconds", "0"],
            vec!["preview", "artifact", "--max-seconds", "3601"],
            vec!["preview", "artifact", "--max-seconds", "nope"],
            vec!["preview", "artifact", "--extra"],
            vec!["preview", "artifact", "--max-seconds"],
            vec![
                "__preview-worker",
                "C:\\artifact.wr",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "extra",
            ],
        ] {
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
        let too_many = (0..17).map(|_| "x").collect::<Vec<_>>();
        assert!(parse_args(&too_many).is_err());
    }

    #[test]
    fn preview_and_worker_parse_bounds() {
        assert!(matches!(
            parse_args(&["preview", "artifact"]).unwrap(),
            Command::Preview {
                max_seconds: 900,
                ..
            }
        ));
        assert!(
            parse_args(&[
                "__preview-worker",
                "relative.wr",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "C:\\slot.lock",
                "00000000000000000000000000000000",
                "10"
            ])
            .is_err()
        );
        assert!(
            parse_args(&[
                "__preview-worker",
                "C:\\artifact.wr",
                "ABC0000000000000000000000000000000000000000000000000000000000000",
                "C:\\slot.lock",
                "00000000000000000000000000000000",
                "10"
            ])
            .is_err()
        );
        assert!(matches!(
            parse_args(&[
                "__preview-worker",
                "C:\\artifact.wr",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "C:\\slot.lock",
                "00000000000000000000000000000000",
                "10",
            ]),
            Ok(Command::PreviewWorker {
                max_seconds: 10,
                ..
            })
        ));
        assert!(
            parse_args(&[
                "__preview-worker",
                "C:\\artifact.wr",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "C:\\slot.lock",
                "ABC000000000000000000000000000000",
                "10",
            ])
            .is_err()
        );
    }
}
