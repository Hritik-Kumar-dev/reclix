//! Global keyboard shortcut registration via the XDG GlobalShortcuts portal.
//!
//! # Wayland reality check
//! ─────────────────────────────────────────────────────────────────────────────
//! The XDG GlobalShortcuts portal (xdg-desktop-portal ≥ 1.12, GNOME Shell ≥ 44)
//! lets an app register shortcuts that fire even when the app has no window
//! focus.  We use it for **Super+Shift+R** (start) and
//! **Super+Shift+Space** (pause/resume).
//!
//! **Bare `Space`** cannot be registered globally on Wayland: the compositor
//! owns the keyboard when no window is focused and GNOME will not forward bare
//! printable/navigation keys to avoid conflicts.  Our default is
//! `Super+Shift+Space`, which works reliably.
//!
//! ## Fallback for compositors without portal support
//! If the portal is unavailable we log a warning and emit GNOME gsettings
//! instructions for adding custom keybindings that call `reclix start` /
//! `reclix pause`.
//!
//! # X11
//! On X11 the portal is unavailable; an XGrab-based fallback is planned for
//! V0.2.  For now the user is directed to the GNOME custom keybindings UI.

use tokio::sync::mpsc;
use tracing::{info, warn};

/// Events the shortcut listener produces.
#[derive(Debug, Clone, Copy)]
pub enum ShortcutEvent {
    Start,
    PauseResume,
    Finish,
}

/// Attempt to register global shortcuts via the XDG portal.
///
/// Returns `Some(receiver)` on success.  Returns `None` if the portal is
/// unavailable — the caller should show fallback setup instructions.
pub async fn register_global_shortcuts(
    start_shortcut: &str,
    pause_resume_shortcut: &str,
) -> Option<mpsc::UnboundedReceiver<ShortcutEvent>> {
    match try_register(start_shortcut, pause_resume_shortcut).await {
        Ok(rx) => {
            info!("global shortcuts registered via XDG GlobalShortcuts portal");
            Some(rx)
        }
        Err(e) => {
            warn!(
                "XDG GlobalShortcuts portal unavailable: {e}\n\
                 ╔═ Shortcut fallback ════════════════════════════════════════╗\n\
                 ║  Add GNOME custom keybindings (run in a terminal):         ║\n\
                 ║                                                            ║\n\
                 ║  BASE=/org/gnome/settings-daemon/plugins/media-keys        ║\n\
                 ║  CB=$BASE/custom-keybindings                               ║\n\
                 ║                                                            ║\n\
                 ║  # 1. Register the keybinding slots                       ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \\\n\
                 ║    \"['$CB/reclix0/', '$CB/reclix1/']\"                     ║\n\
                 ║                                                            ║\n\
                 ║  # 2. Start shortcut: Super+Shift+R → reclix start        ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ name    'Reclix Start'   ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ command 'reclix start'   ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ binding '<Super><Shift>r'║\n\
                 ║                                                            ║\n\
                 ║  # 3. Pause/resume shortcut: Super+Shift+Space            ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ name    'Reclix Pause'   ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ command 'reclix pause'   ║\n\
                 ║  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ binding '<Super><Shift>space'║\n\
                 ║                                                            ║\n\
                 ║  Or use the tray icon / `reclix` CLI directly.            ║\n\
                 ╚════════════════════════════════════════════════════════════╝"
            );
            None
        }
    }
}

async fn try_register(
    start_shortcut: &str,
    pause_resume_shortcut: &str,
) -> anyhow::Result<mpsc::UnboundedReceiver<ShortcutEvent>> {
    use anyhow::Context;
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    let proxy = GlobalShortcuts::new()
        .await
        .context("GlobalShortcuts portal: could not create proxy")?;

    let session = proxy
        .create_session()
        .await
        .context("GlobalShortcuts: create_session failed")?;

    let shortcuts = vec![
        NewShortcut::new("reclix-start", "Start recording")
            .preferred_trigger(start_shortcut),
        NewShortcut::new("reclix-pause", "Pause / resume recording")
            .preferred_trigger(pause_resume_shortcut),
        NewShortcut::new("reclix-finish", "Finish recording")
            .preferred_trigger("Escape"),
    ];

    proxy
        .bind_shortcuts(&session, &shortcuts, &ashpd::WindowIdentifier::default())
        .await
        .context("GlobalShortcuts: bind_shortcuts failed")?;

    let (tx, rx) = mpsc::unbounded_channel();

    // `receive_activated` returns an async stream of `Activated` signals.
    let mut stream = proxy
        .receive_activated()
        .await
        .context("GlobalShortcuts: receive_activated failed")?;

    tokio::spawn(async move {
        // Keep session alive as long as the task runs.
        let _session = session;
        while let Some(activation) = stream.next().await {
            let ev = match activation.shortcut_id() {
                "reclix-start"  => Some(ShortcutEvent::Start),
                "reclix-pause"  => Some(ShortcutEvent::PauseResume),
                "reclix-finish" => Some(ShortcutEvent::Finish),
                other => {
                    warn!("unknown shortcut id: {other}");
                    None
                }
            };
            if let Some(ev) = ev {
                if tx.send(ev).is_err() {
                    break; // service dropped the receiver → shut down
                }
            }
        }
    });

    Ok(rx)
}
