//! `ksni` 0.3 based StatusNotifierItem tray indicator.

use std::sync::{Arc, Mutex};

use ksni::{menu::StandardItem, Handle, MenuItem, Tray, TrayMethods};
use tokio::sync::mpsc;
use tracing::{error, info};

use reclix_core::state::RecordingState;

use crate::icon::{icon_name, tray_title};

// ─── Commands the tray sends back to the service ──────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    PauseResume,
    Finish,
    ToggleMic,
    ToggleSystemAudio,
    OpenSettings,
}

// ─── Shared state written by the service, read by the tray ───────────────────

#[derive(Debug, Clone)]
pub struct TrayState {
    pub recording_state:   RecordingState,
    pub elapsed_secs:      u64,
    pub mic_enabled:       bool,
    pub sys_audio_enabled: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            recording_state:   RecordingState::Idle,
            elapsed_secs:      0,
            mic_enabled:       true,
            sys_audio_enabled: false,
        }
    }
}

// ─── ksni Tray implementation ─────────────────────────────────────────────────

/// The tray struct holds a snapshot of state (copied from the shared Arc on
/// each menu rebuild) and a sender for commands back to the service.
pub struct ReclixTray {
    state:   Arc<Mutex<TrayState>>,
    cmd_tx:  mpsc::UnboundedSender<TrayCommand>,
}

impl ReclixTray {
    pub fn new(
        state:  Arc<Mutex<TrayState>>,
        cmd_tx: mpsc::UnboundedSender<TrayCommand>,
    ) -> Self {
        Self { state, cmd_tx }
    }

    fn send(&self, cmd: TrayCommand) {
        if let Err(e) = self.cmd_tx.send(cmd) {
            error!("tray → service channel error: {e}");
        }
    }

    fn current_state(&self) -> TrayState {
        self.state.lock().unwrap().clone()
    }
}

impl Tray for ReclixTray {
    fn id(&self) -> String {
        "reclix".into()
    }

    fn icon_name(&self) -> String {
        icon_name(self.current_state().recording_state).into()
    }

    fn title(&self) -> String {
        let st = self.current_state();
        tray_title(st.recording_state, st.elapsed_secs)
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let st = self.current_state();
        let is_recording = st.recording_state == RecordingState::Recording;
        let is_paused    = st.recording_state == RecordingState::Paused;
        let is_active    = is_recording || is_paused;

        let mut items: Vec<MenuItem<Self>> = Vec::new();

        // ── Pause / Resume ────────────────────────────────────────────────────
        if is_recording {
            items.push(
                StandardItem {
                    label:   "⏸  Pause".into(),
                    enabled: true,
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::PauseResume)),
                    ..Default::default()
                }
                .into(),
            );
        } else if is_paused {
            items.push(
                StandardItem {
                    label:   "▶  Resume".into(),
                    enabled: true,
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::PauseResume)),
                    ..Default::default()
                }
                .into(),
            );
        }

        // ── Finish ────────────────────────────────────────────────────────────
        if is_active {
            items.push(
                StandardItem {
                    label:   "⏹  Finish & Export".into(),
                    enabled: true,
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::Finish)),
                    ..Default::default()
                }
                .into(),
            );
        }

        if is_active {
            items.push(MenuItem::Separator);
        }

        // ── Mic toggle ────────────────────────────────────────────────────────
        let mic_on = st.mic_enabled;
        let mic_label = if mic_on { "🎤  Mic  ✓" } else { "🎤  Mic  ✗" };
        items.push(
            StandardItem {
                label:   mic_label.into(),
                enabled: is_active,
                activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::ToggleMic)),
                ..Default::default()
            }
            .into(),
        );

        // ── System audio (V0.2+, disabled) ────────────────────────────────────
        items.push(
            StandardItem {
                label:   "🔈  System Audio (V0.2)".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        // ── Shortcut cheat-sheet ──────────────────────────────────────────────
        for label in [
            "Super+Shift+R  Start",
            "Super+Shift+Space  Pause/Resume",
            "Esc  Finish",
        ] {
            items.push(
                StandardItem {
                    label:   label.into(),
                    enabled: false,
                    activate: Box::new(|_| {}),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);

        items.push(
            StandardItem {
                label:   "⚙  Settings…".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::OpenSettings)),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

// ─── spawn helper ─────────────────────────────────────────────────────────────

/// Spawn the tray asynchronously using `ksni` 0.3's async API.
///
/// Returns:
/// * `Arc<Mutex<TrayState>>` — write to update the indicator; call
///   `handle.update(|_| true)` afterwards to force a refresh.
/// * `Handle<ReclixTray>` — call `.update(|tray| { …; true })` to push state
/// * `mpsc::UnboundedReceiver<TrayCommand>` — user menu actions
pub async fn spawn_tray() -> anyhow::Result<(
    Arc<Mutex<TrayState>>,
    Handle<ReclixTray>,
    mpsc::UnboundedReceiver<TrayCommand>,
)> {
    let state = Arc::new(Mutex::new(TrayState::default()));
    let state_clone = Arc::clone(&state);

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let tray = ReclixTray::new(state_clone, cmd_tx);

    let handle = tray
        .spawn()
        .await
        .map_err(|e| anyhow::anyhow!("failed to spawn tray: {e}"))?;

    info!("tray spawned via StatusNotifierItem");
    Ok((state, handle, cmd_rx))
}
