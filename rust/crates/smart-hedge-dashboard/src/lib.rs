//! Rust port of `python/smart_hedge/dashboard.py`: a local-only,
//! read-only debug console over the deterministic engine. The Python
//! original uses FastAPI/uvicorn; this hand-rolls a minimal HTTP/1.1
//! server instead of depending on either — see `http`'s module doc
//! comment for why that's a safe thing to hand-roll here (unlike the
//! *client* side, which does depend on `ureq`/`rustls` — see
//! `smart-hedge-data`'s `Cargo.toml`).

pub mod cache;
pub mod html;
pub mod http;
pub mod routes;
pub mod server;

#[cfg(test)]
mod integration_tests;

pub use routes::AppState;
pub use server::{bind, build_state, run, serve, DashboardError};
