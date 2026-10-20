mod content;
use crate::cli::Command;
use std::process::ExitCode;

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Help => {
            println!("Worldwright developer CLI");
            println!("Usage: worldwright <--help|--version|status>");
            println!("       worldwright content validate <path>");
            println!("Local JSON validation only; no network service, deployment, or durability.");
        }
        Command::ContentHelp => {
            println!("Usage: worldwright content validate <path>");
            println!("Validate schema v1 content and preflight room/exit construction.");
            println!("Exit codes: 0 valid, 1 file/content failure, 2 usage error.");
            println!(
                "Paths are relative to your current working directory; quote paths containing spaces."
            );
        }
        Command::Version => println!("worldwright {}", env!("CARGO_PKG_VERSION")),
        Command::Status => {
            println!(
                "worldwright: local JSON content validation implemented; no network service or durability"
            );
            println!("SDK: {}", worldwright_sdk::STATUS);
        }
        Command::Validate(path) => return content::validate(&path),
    }
    ExitCode::SUCCESS
}
