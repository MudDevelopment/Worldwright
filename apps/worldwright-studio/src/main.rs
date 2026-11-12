use std::process::ExitCode;
use worldwright_studio::{
    cli::{Command, parse},
    workspace::Workspace,
};

#[tokio::main]
async fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1)) {
        Ok(Command::Help) => {
            println!("Worldwright Studio");
            println!(
                "Usage: worldwright-studio serve --workspace <path> [--content <path>] [--bind 127.0.0.1:PORT] [--preview-command <path>] [--release-endpoint http://127.0.0.1:PORT] [--operator-token-env ENV | --operator-token-file ABSOLUTE_PATH]"
            );
            println!("Draft revisions are append-only and use shared authoritative validation.");
            println!(
                "Preview runs the isolated worldwright preview supervisor; publication uses the authenticated loopback operator API when configured."
            );
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("worldwright-studio {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Status) => {
            println!(
                "Worldwright Studio draft editing, shared validation, immutable packaging, isolated preview, and controlled publication requests are implemented"
            );
            ExitCode::SUCCESS
        }
        Ok(Command::Serve(options)) => {
            match Workspace::open_or_create(&options.workspace, options.content.as_deref()) {
                Ok(workspace) => match worldwright_studio::web::serve(options, workspace).await {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("worldwright-studio: {error}");
                        ExitCode::from(1)
                    }
                },
                Err(_) => {
                    eprintln!("worldwright-studio: workspace initialization failed");
                    ExitCode::from(1)
                }
            }
        }
        Err(error) => {
            eprintln!("worldwright-studio: {error}");
            ExitCode::from(2)
        }
    }
}
