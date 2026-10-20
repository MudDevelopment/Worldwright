#[tokio::main]
async fn main() -> std::process::ExitCode {
    worldwright_server::run_from_env().await
}
