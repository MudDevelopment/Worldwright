mod cli;
mod fixture;
mod report;
mod runner;

use cli::Command;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(Command::Help) => {
            cli::print_help();
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("worldwright-load {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Generate(options)) => fixture::generate(options),
        Ok(Command::Run(options)) => runner::run(options).await,
        Err(message) => {
            eprintln!("worldwright-load: {message}");
            ExitCode::from(2)
        }
    }
}
