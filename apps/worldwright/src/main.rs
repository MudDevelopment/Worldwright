//! Developer CLI placeholder. No project generation or validation commands yet.

use std::ffi::OsStr;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if arguments.next().is_some() {
        eprintln!("worldwright: expected one of --help, --version, or status");
        return ExitCode::from(2);
    }

    match first.as_deref().and_then(OsStr::to_str) {
        Some("--help" | "-h") => {
            println!("Worldwright developer CLI (scaffold only)");
            println!("Usage: worldwright <--help|--version|status>");
            println!("No project generation, content validation, or deployment exists yet.");
        }
        Some("--version" | "-V") => {
            println!("worldwright {}", env!("CARGO_PKG_VERSION"));
        }
        Some("status") => {
            println!("worldwright: scaffold only; no development commands implemented");
            println!("SDK: {}", worldwright_sdk::STATUS);
        }
        _ => {
            eprintln!("worldwright: operation not implemented; use --help (scaffold only)");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
