mod content;
mod maturity;
mod play;
mod play_input;
mod release;
use crate::cli::Command;
use std::process::ExitCode;

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Help => {
            println!("Worldwright developer CLI");
            println!("Usage: worldwright <--help|--version|status>");
            println!("       worldwright content validate <path>");
            println!(
                "       worldwright content package <content-path> --output <artifact-path> --release-id <release-id> --source-revision <revision> --builder <identity>"
            );
            println!("       worldwright content inspect <artifact-path> [--json]");
            println!(
                "       worldwright content migrate <input> --output <path> [--backup <path>] [--dry-run]"
            );
            println!(
                "       worldwright project new <path> --name <package> [--template minimal|expedition] [--dry-run]"
            );
            println!("       worldwright compatibility [--json]");
            println!(
                "       worldwright benchmark [--clients 100] [--commands-per-client 10] [--json]"
            );
            println!("       worldwright play <path>");
            println!("       worldwright preview <artifact-path> [--max-seconds <1..=3600>]");
            println!(
                "Packaging and isolated preview are implemented; publication and activation are server-side."
            );
            println!("Preview has no persistence or production credentials.");
            println!("Preview capacity: 4 concurrent previews; maximum lifetime: 3600 seconds.");
            println!("no network service is started by these developer commands.");
        }
        Command::ContentHelp => {
            println!("Usage: worldwright content validate <path>");
            println!(
                "       worldwright content package <content-path> --output <artifact-path> --release-id <release-id> --source-revision <revision> --builder <identity>"
            );
            println!("       worldwright content inspect <artifact-path> [--json]");
            println!(
                "       worldwright content migrate <input> --output <path> [--backup <path>] [--dry-run]"
            );
            println!("Validate schema v1 content and preflight room/exit construction.");
            println!(
                "Package creates immutable deterministic artifacts; inspect reports manifest and content metadata."
            );
            println!(
                "Exit codes: 0 success, 1 validation/I/O failure, 2 usage error, 3 immutable conflict."
            );
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
        Command::PreviewHelp => {
            println!("Usage: worldwright preview <artifact-path> [--max-seconds <1..=3600>]");
            println!("Run a validated release artifact in an isolated local preview.");
            println!("Preview has no persistence or production credentials.");
            println!("Preview capacity: 4 concurrent previews; maximum lifetime: 3600 seconds.");
            println!(
                "Exit codes: 0 clean quit/EOF, 1 validation, runtime, or timeout failure, 2 usage error."
            );
        }
        Command::Version => println!("worldwright {}", env!("CARGO_PKG_VERSION")),
        Command::Status => {
            println!(
                "worldwright: content validation, immutable packaging, and isolated preview implemented"
            );
            println!("Local play remains available through `worldwright play <path>`.");
            println!("Publication and activation are server-side.");
            println!("Preview has no persistence or production credentials.");
            println!("Preview capacity: 4 concurrent previews; maximum lifetime: 3600 seconds.");
            println!("no network service is started by these developer commands.");
            println!("SDK: {}", worldwright_sdk::STATUS);
        }
        Command::Compatibility { json } => return maturity::compatibility(json),
        Command::Benchmark {
            clients,
            commands_per_client,
            json,
        } => return maturity::benchmark(clients, commands_per_client, json),
        Command::ProjectNew {
            path,
            name,
            template,
            dry_run,
        } => return maturity::project_new(&path, &name, &template, dry_run),
        Command::Validate(path) => return content::validate(&path),
        Command::ContentMigrate {
            input,
            output,
            backup,
            dry_run,
        } => return maturity::migrate(&input, &output, backup.as_deref(), dry_run),
        Command::Play(path) => return play::play(&path),
        Command::ContentPackage {
            content,
            output,
            release_id,
            source_revision,
            builder_identity,
        } => {
            return release::package(
                &content,
                &output,
                release_id,
                source_revision,
                builder_identity,
            );
        }
        Command::ContentInspect { artifact, json } => return release::inspect(&artifact, json),
        Command::Preview {
            artifact,
            max_seconds,
        } => return release::preview(&artifact, max_seconds),
        Command::PreviewWorker {
            artifact,
            expected_digest,
            slot,
            token,
            max_seconds,
        } => {
            return release::preview_worker(&artifact, expected_digest, &slot, &token, max_seconds);
        }
    }
    ExitCode::SUCCESS
}
