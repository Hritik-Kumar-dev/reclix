//! D-Bus client — thin wrapper around zbus proxy calls.

use anyhow::Context;
use zbus::{proxy, Connection};

use reclix_core::dbus_types::{ServiceCommand, ServiceStatus};

// ── zbus proxy (auto-generated from interface name) ───────────────────────────

#[proxy(
    interface  = "org.reclix.Service1",
    default_service  = "org.reclix.Service1",
    default_path = "/org/reclix/Service1"
)]
trait ReclixService {
    async fn get_status(&self) -> zbus::Result<String>;
    async fn send_command(&self, command_json: &str) -> zbus::Result<()>;
}

// ─── public helpers ───────────────────────────────────────────────────────────

async fn connect() -> anyhow::Result<ReclixServiceProxy<'static>> {
    let conn = Connection::session()
        .await
        .context("could not connect to D-Bus session bus")?;
    ReclixServiceProxy::new(&conn)
        .await
        .context("could not create D-Bus proxy for org.reclix.Service1")
}

/// Fetch the current [`ServiceStatus`] from the running service.
pub async fn get_status() -> anyhow::Result<ServiceStatus> {
    let proxy = connect().await?;
    let json = proxy
        .get_status()
        .await
        .context("GetStatus D-Bus call failed")?;
    serde_json::from_str(&json).context("parsing ServiceStatus JSON")
}

/// Send a [`ServiceCommand`] and return the updated [`ServiceStatus`].
pub async fn send_command(cmd: ServiceCommand) -> anyhow::Result<ServiceStatus> {
    let proxy = connect().await?;
    let json = serde_json::to_string(&cmd)
        .context("serialising ServiceCommand")?;
    proxy
        .send_command(&json)
        .await
        .context("SendCommand D-Bus call failed")?;
    // Fetch updated status immediately after.
    let status_json = proxy
        .get_status()
        .await
        .context("GetStatus after command failed")?;
    serde_json::from_str(&status_json).context("parsing updated ServiceStatus")
}
