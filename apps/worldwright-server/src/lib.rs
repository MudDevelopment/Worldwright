mod application;
mod auth;
mod cli;
mod web;
mod world;

use std::{error::Error, fmt, future::Future, net::SocketAddr, path::PathBuf, process::ExitCode};

pub use application::BoundServer;

pub struct ServerConfig {
    pub bind: SocketAddr,
    pub allowed_origin: String,
    pub content_path: PathBuf,
    pub accounts_path: PathBuf,
    pub allow_insecure_http: bool,
}

#[derive(Debug)]
pub enum ServerError {
    Configuration(&'static str),
    Startup(String),
    Io(std::io::Error),
    Runtime(&'static str),
}

impl ServerError {
    pub(crate) fn startup(message: &'static str) -> Self {
        Self::Startup(message.to_owned())
    }

    pub(crate) fn startup_record(index: usize, message: &'static str) -> Self {
        Self::Startup(format!("account registry record {index}: {message}"))
    }

    pub(crate) fn io(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(formatter, "configuration error: {message}"),
            Self::Startup(message) => write!(formatter, "startup error: {message}"),
            Self::Io(_) => formatter.write_str("server I/O error"),
            Self::Runtime(message) => write!(formatter, "server runtime error: {message}"),
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

pub async fn run_from_env() -> ExitCode {
    cli::run(std::env::args_os().skip(1)).await
}

impl BoundServer {
    pub async fn serve<F>(self, shutdown: F) -> Result<(), ServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        application::serve(self, shutdown).await
    }
}
