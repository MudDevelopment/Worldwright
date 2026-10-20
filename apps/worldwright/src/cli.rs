use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    ContentHelp,
    Version,
    Status,
    Validate(PathBuf),
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    // The command has at most three arguments. Do not collect an unbounded list.
    let mut args = args.into_iter();
    let first = args.next();
    let second = args.next();
    let third = args.next();
    if args.next().is_some() {
        return Err("too many arguments");
    }
    match (
        first.as_deref().and_then(|v| v.to_str()),
        second.as_deref().and_then(|v| v.to_str()),
        third,
    ) {
        (Some("--help" | "-h"), None, None) if second.is_none() => Ok(Command::Help),
        (Some("--version" | "-V"), None, None) if second.is_none() => Ok(Command::Version),
        (Some("status"), None, None) if second.is_none() => Ok(Command::Status),
        (Some("content"), Some("--help" | "-h"), None) => Ok(Command::ContentHelp),
        (Some("content"), Some("validate"), Some(path)) if path == "--help" || path == "-h" => {
            Ok(Command::ContentHelp)
        }
        (Some("content"), Some("validate"), Some(path)) if !path.is_empty() => {
            Ok(Command::Validate(path.into()))
        }
        _ => Err("expected --help, --version, status, or content validate <path>"),
    }
}
