//! Builder application placeholder. No UI, preview process, or file editor yet.

use std::ffi::OsStr;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if arguments.next().is_some() {
        eprintln!("worldwright-studio: expected one of --help, --version, or status");
        return ExitCode::from(2);
    }

    match first.as_deref().and_then(OsStr::to_str) {
        Some("--help" | "-h") => {
            println!("Worldwright Studio (scaffold only)");
            println!("Usage: worldwright-studio <--help|--version|status>");
            println!("No editor, content publication, or graphical window exists yet.");
        }
        Some("--version" | "-V") => {
            println!("worldwright-studio {}", env!("CARGO_PKG_VERSION"));
        }
        Some("status") => {
            println!("worldwright-studio: scaffold only; no builder UI implemented");
            println!("Protocol: {}", worldwright_protocol::STATUS);
        }
        _ => {
            eprintln!("worldwright-studio: operation not implemented; use --help (scaffold only)");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
