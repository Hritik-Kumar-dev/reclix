//! XDG ScreenCast portal negotiation via `ashpd`.
//!
//! Wayland note
//! ─────────────
//! On Wayland the portal *must* be driven from a thread that is **not** the
//! GLib/GTK main thread.  We run it in a dedicated tokio task.  The returned
//! `ScreenCastSource` carries the PipeWire `node_id` and the raw file
//! descriptor that must be passed to `pipewire_capture::start_capture`.

use std::os::fd::OwnedFd;

use anyhow::Context;
use ashpd::{
    desktop::{
        screencast::{CursorMode, PersistMode, Screencast, SourceType},
        ResponseError,
    },
    WindowIdentifier,
};
use tracing::{debug, info, warn};

/// Result of a successful portal negotiation.
pub struct ScreenCastSource {
    /// PipeWire node ID for the selected screen stream.
    pub node_id: u32,
    /// PipeWire remote fd — pass to `pw::Context`.
    pub fd: OwnedFd,
    /// Reported stream width (may be 0 if portal doesn't supply it).
    pub width: u32,
    /// Reported stream height.
    pub height: u32,
}

/// Ask the XDG ScreenCast portal to share a screen and return the PipeWire
/// source.
///
/// This is `async` because `ashpd` uses D-Bus under the hood.
/// Must be called from a tokio context.
///
/// # Errors
/// Returns [`reclix_core::error::ReclixError::PortalDenied`] when the user
/// dismisses the portal dialog or the portal is unavailable.
pub async fn request_screencast() -> anyhow::Result<ScreenCastSource> {
    info!("requesting screen-cast via XDG portal");

    let proxy = Screencast::new()
        .await
        .context("could not connect to XDG ScreenCast portal")?;

    let session = proxy
        .create_session()
        .await
        .context("ScreenCast: create_session failed")?;

    // Request full-screen (monitor) source.
    // CursorMode::Metadata lets us read cursor position for focus mode (V0.4).
    proxy
        .select_sources(
            &session,
            CursorMode::Embedded,   // embed cursor in stream for V0.1
            SourceType::Monitor.into(),
            false,                  // multiple = false → single monitor
            None,
            PersistMode::DoNot,
        )
        .await
        .context("ScreenCast: select_sources failed")?;

    let response = proxy
        .start(&session, &WindowIdentifier::default())
        .await
        .context("ScreenCast: start failed")?;

    let streams = response
        .response()
        .map_err(|e| match e {
            ResponseError::Cancelled => {
                anyhow::anyhow!("user cancelled the screen-cast dialog")
            }
            ResponseError::Other => {
                anyhow::anyhow!("screen-cast portal returned an error response")
            }
        })?
        .streams()
        .to_vec();

    let stream = streams
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("portal returned no streams"))?;

    let node_id = stream.pipe_wire_node_id();
    let (width, height) = stream
        .size()
        .map(|(w, h)| (w as u32, h as u32))
        .unwrap_or((0, 0));

    debug!(node_id, width, height, "portal stream selected");

    let fd = proxy
        .open_pipe_wire_remote(&session)
        .await
        .context("ScreenCast: open_pipe_wire_remote failed")?;

    info!(node_id, "screen-cast portal ready");

    Ok(ScreenCastSource { node_id, fd, width, height })
}
