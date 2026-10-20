//! Local developer CLI. Content validation does not start a game service.
mod cli;
mod commands;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(command) => commands::run(command),
        Err(message) => {
            eprintln!("worldwright: {message}; use --help");
            ExitCode::from(2)
        }
    }
}
