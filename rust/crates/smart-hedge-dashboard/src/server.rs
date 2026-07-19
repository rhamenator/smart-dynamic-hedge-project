use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use smart_hedge_config::LoadedConfig;
use smart_hedge_engine::{EngineError, SmartHedgeEngine};

use crate::cache::Cache;
use crate::http::read_request;
use crate::routes::{handle, AppState};

#[derive(Debug)]
pub enum DashboardError {
    Engine(EngineError),
    Io(io::Error),
}

impl std::fmt::Display for DashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Engine(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DashboardError {}

impl From<EngineError> for DashboardError {
    fn from(e: EngineError) -> Self {
        DashboardError::Engine(e)
    }
}

impl From<io::Error> for DashboardError {
    fn from(e: io::Error) -> Self {
        DashboardError::Io(e)
    }
}

/// Port of `dashboard.create_app`'s setup (minus the FastAPI/uvicorn
/// framework itself): builds the engine and the recommendation cache.
pub fn build_state(loaded: LoadedConfig, project_root: PathBuf, cpp_source: PathBuf) -> Result<AppState, DashboardError> {
    let cache_seconds = loaded.config.dashboard.cache_seconds;
    let engine = SmartHedgeEngine::new(loaded, project_root, cpp_source)?;
    Ok(AppState { engine, cache: Cache::new(cache_seconds) })
}

pub fn bind(host: &str, port: u16) -> io::Result<TcpListener> {
    TcpListener::bind((host, port))
}

fn handle_connection(state: &AppState, stream: TcpStream) {
    match read_request(&stream) {
        Ok(req) => {
            let response = handle(state, &req);
            let _ = crate::http::write_response(&stream, response.status, response.reason, response.content_type, &response.body);
        }
        Err(_) => {
            let _ = crate::http::write_response(&stream, 400, "Bad Request", "text/plain; charset=utf-8", b"malformed request");
        }
    }
}

/// Accepts connections forever, one thread per connection — appropriate
/// for a local-only, low-traffic debug dashboard (SDH-HLR-150), not a
/// production web server under real concurrent load. Returns only if
/// `accept` itself errors (e.g. the listener was closed).
pub fn run(listener: TcpListener, state: AppState) -> io::Result<()> {
    let state = Arc::new(state);
    loop {
        let (stream, _) = listener.accept()?;
        let state = Arc::clone(&state);
        std::thread::spawn(move || handle_connection(&state, stream));
    }
}

/// Convenience wrapper matching `cli.py`'s `cmd_serve`: build the engine,
/// bind the configured (or overridden) host/port, and serve forever.
pub fn serve(
    loaded: LoadedConfig,
    project_root: PathBuf,
    cpp_source: PathBuf,
    host_override: Option<&str>,
    port_override: Option<u16>,
) -> Result<(), DashboardError> {
    let host = host_override.map(str::to_string).unwrap_or_else(|| loaded.config.dashboard.host.clone());
    let port = port_override.unwrap_or_else(|| loaded.config.dashboard.port.clamp(1, u16::MAX as i64) as u16);
    let state = build_state(loaded, project_root, cpp_source)?;
    let listener = bind(&host, port)?;
    // Query the listener for the actually-bound port rather than echoing
    // back the requested one — `--port 0` asks the OS to pick an ephemeral
    // port, so the requested and actual values can differ.
    let bound_port = listener.local_addr()?.port();
    println!("smart-hedge dashboard listening on http://{host}:{bound_port}");
    // stdout is block-buffered when not connected to a terminal (e.g. a
    // parent process piping it to detect readiness) — flush explicitly so
    // that line is actually visible before `run` blocks forever.
    io::stdout().flush()?;
    run(listener, state)?;
    Ok(())
}
