//! Operator application placeholder. No privileged operations or network access.

use std::ffi::OsStr;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if arguments.next().is_some() {
        eprintln!("worldwright-console: expected one of --help, --version, or status");
        return ExitCode::from(2);
    }

    match first.as_deref().and_then(OsStr::to_str) {
        Some("--help" | "-h") => {
            println!("Worldwright Console (scaffold only)");
            println!("Usage: worldwright-console <--help|--version|status>");
            println!("No admin UI, repair, moderation, or database access exists yet.");
        }
        Some("--version" | "-V") => {
            println!("worldwright-console {}", env!("CARGO_PKG_VERSION"));
        }
        Some("status") => {
            println!("worldwright-console: scaffold only; no operator UI implemented");
            println!("Protocol: {}", worldwright_protocol::STATUS);
        }
        _ => {
            eprintln!("worldwright-console: operation not implemented; use --help (scaffold only)");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
