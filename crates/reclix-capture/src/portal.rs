//! XDG ScreenCast portal negotiation via `ashpd` 0.10.

use std::os::fd::OwnedFd;

use anyhow::Context;
use ashpd::{
    desktop::{
        screencast::{CursorMode, Screencast, SourceType},
        PersistMode,
    },
    WindowIdentifier,
};
use tracing::{debug, info};

/// Result of a successful portal negotiation.
pub struct ScreenCastSource {
    /// PipeWire node ID for the selected screen stream.
    pub node_id: u32,
    /// PipeWire remote fd — pass to the PipeWire context.
    pub fd: OwnedFd,
    /// Reported stream width in pixels (may be 0 if portal omits it).
    pub width: u32,
    /// Reported stream height in pixels.
    pub height: u32,
}

/// Ask the XDG ScreenCast portal to share a screen and return the PipeWire
/// source.  Must be called from a tokio context.
pub async fn request_screencast() -> anyhow::Result<ScreenCastSource> {
    info!("requesting screen-cast via XDG portal");

    let proxy = Screencast::new()
        .await
        .context("could not connect to XDG ScreenCast portal")?;

    let session = proxy
        .create_session()
        .await
        .context("ScreenCast: create_session failed")?;

    // Embed cursor in stream for V0.1 (Metadata is for V0.4 focus mode).
    proxy
        .select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor.into(),
            false,      // single monitor
            None,       // no restore token
            PersistMode::DoNot,
        )
        .await
        .context("ScreenCast: select_sources failed")?;

    let response = proxy
        .start(&session, None)
        .await
        .context("ScreenCast: start failed")?
        .response()
        .context("ScreenCast: user cancelled or portal error")?;

    let stream = response
        .streams()
        .iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("portal returned no streams"))?;

    let node_id = stream.pipe_wire_node_id();
    let (width, height) = stream
        .size()
        .map(|(w, h)| (w.max(0) as u32, h.max(0) as u32))
        .unwrap_or((0, 0));

    debug!(node_id, width, height, "portal stream selected");

    let fd = proxy
        .open_pipe_wire_remote(&session)
        .await
        .context("ScreenCast: open_pipe_wire_remote failed")?;

    info!(node_id, "screen-cast portal ready");
    Ok(ScreenCastSource { node_id, fd, width, height })
}
