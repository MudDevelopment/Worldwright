//! Server entry point. Does not bind a port, load secrets, or serve players.

mod application;

use std::ffi::OsStr;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if arguments.next().is_some() {
        eprintln!("worldwright-server: expected one of --help, --version, or status");
        return ExitCode::from(2);
    }

    match first.as_deref().and_then(OsStr::to_str) {
        Some("--help" | "-h") => {
            println!("Worldwright server (scaffold only)");
            println!("Usage: worldwright-server <--help|--version|status>");
            println!("No listener, world runtime, authentication, or database exists yet.");
        }
        Some("--version" | "-V") => {
            println!("worldwright-server {}", env!("CARGO_PKG_VERSION"));
        }
        Some("status") => {
            println!("worldwright-server: scaffold only; not ready to serve players");
            println!("Protocol: {}", worldwright_protocol::STATUS);
            println!("SDK: {}", worldwright_sdk::STATUS);
        }
        None if first.is_none() => {
            if let Err(error) = application::start() {
                eprintln!("worldwright-server: {error}");
                return ExitCode::from(2);
            }
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("worldwright-server: operation not implemented; use --help (scaffold only)");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
