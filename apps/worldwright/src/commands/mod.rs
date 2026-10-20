mod content;
mod play;
mod play_input;
use crate::cli::Command;
use std::process::ExitCode;

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Help => {
            println!("Worldwright developer CLI");
            println!("Usage: worldwright <--help|--version|status>");
            println!("       worldwright content validate <path>");
            println!("       worldwright play <path>");
            println!(
                "Local JSON validation and single-player play; no network service, deployment, or durability."
            );
        }
        Command::ContentHelp => {
            println!("Usage: worldwright content validate <path>");
            println!("Validate schema v1 content and preflight room/exit construction.");
            println!("Exit codes: 0 valid, 1 file/content failure, 2 usage error.");
            println!(
                "Paths are relative to your current working directory; quote paths containing spaces."
            );
        }
        Command::PlayHelp => {
            println!("Usage: worldwright play <path>");
            println!("Start a single-player, in-memory session from validated JSON content.");
            println!("Commands: look (l), go <exit-label>, say <text>, help, quit (exit).");
            println!("EOF ends cleanly. No login, network service, or persistence.");
            println!("Exit codes: 0 clean quit/EOF, 1 startup/session failure, 2 usage error.");
        }
        Command::Version => println!("worldwright {}", env!("CARGO_PKG_VERSION")),
        Command::Status => {
            println!(
                "worldwright: local JSON content validation and single-player play implemented; no network service or durability"
            );
            println!("SDK: {}", worldwright_sdk::STATUS);
        }
        Command::Validate(path) => return content::validate(&path),
        Command::Play(path) => return play::play(&path),
    }
    ExitCode::SUCCESS
}
