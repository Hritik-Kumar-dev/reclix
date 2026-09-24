//! reclix-service — main entry point.
//!
//! Starts the tokio runtime, initialises tracing, then hands off to
//! `service::run()`.

mod dbus_interface;
mod orchestrator;
mod service;
mod shortcut;
mod notify;

use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise tracing; RECLIX_LOG overrides the default filter.
    fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RECLIX_LOG")
                .unwrap_or_else(|_| EnvFilter::new("reclix=info,warn")),
        )
        .with_target(true)
        .init();

    tracing::info!("reclix-service starting (v{})", env!("CARGO_PKG_VERSION"));

    service::run().await
}
