//! Top-level service runner.

use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::Mutex;
use tracing::{error, info, warn};
use zbus::connection::Builder as ConnectionBuilder;

use reclix_core::config::ReclixConfig;
use reclix_tray::{spawn_tray, TrayCommand};

use crate::{
    dbus_interface::{ReclixDbusService, OBJECT_PATH, SERVICE_NAME},
    orchestrator::Orchestrator,
    shortcut::{register_global_shortcuts, ShortcutEvent},
};

pub async fn run() -> anyhow::Result<()> {
    // 1. Config
    let config = ReclixConfig::load();
    info!(fps = config.fps, prefer_vaapi = config.prefer_vaapi, "config loaded");

    // 2. Tray
    let (tray_state, tray_handle, mut tray_cmd_rx) = match spawn_tray().await {
        Ok(v)  => v,
        Err(e) => {
            warn!("tray unavailable: {e} — running without indicator");
            return run_without_tray(config).await;
        }
    };

    // 3. Orchestrator
    let orchestrator = Arc::new(Mutex::new(Orchestrator::new(
        config.clone(),
        Arc::clone(&tray_state),
    )));

    // 4. D-Bus
    let dbus_service = ReclixDbusService { orchestrator: Arc::clone(&orchestrator) };
    let _conn = ConnectionBuilder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, dbus_service)?
        .build()
        .await?;
    info!("D-Bus service registered as {SERVICE_NAME}");

    // 5. Global shortcuts
    let mut shortcut_rx = register_global_shortcuts(
        &config.shortcuts.start,
        &config.shortcuts.pause_resume,
    )
    .await;

    info!("reclix-service ready");

    // 6. Event loop
    loop {
        tokio::select! {
            cmd = tray_cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                dispatch_tray_cmd(&orchestrator, &tray_handle, cmd).await;
            }
            Some(ev) = async {
                match shortcut_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None     => std::future::pending().await,
                }
            } => {
                dispatch_shortcut_event(&orchestrator, &tray_handle, ev).await;
            }
        }
    }

    info!("reclix-service shutting down");
    Ok(())
}

async fn run_without_tray(config: ReclixConfig) -> anyhow::Result<()> {
    let tray_state = Arc::new(StdMutex::new(reclix_tray::TrayState::default()));
    let orchestrator = Arc::new(Mutex::new(Orchestrator::new(config.clone(), tray_state)));

    let dbus_service = ReclixDbusService { orchestrator: Arc::clone(&orchestrator) };
    let _conn = ConnectionBuilder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, dbus_service)?
        .build()
        .await?;
    info!("D-Bus service registered (no tray)");

    let mut shortcut_rx = register_global_shortcuts(
        &config.shortcuts.start,
        &config.shortcuts.pause_resume,
    )
    .await;

    loop {
        let ev = match shortcut_rx.as_mut() {
            Some(rx) => match rx.recv().await {
                Some(ev) => ev,
                None     => break,
            },
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
                continue;
            }
        };
        let result = match ev {
            ShortcutEvent::Start       => orchestrator.lock().await.handle_start().await,
            ShortcutEvent::PauseResume => orchestrator.lock().await.handle_pause_resume().await,
            ShortcutEvent::Finish      => orchestrator.lock().await.handle_finish().await,
        };
        if let Err(e) = result {
            error!("shortcut error: {e:#}");
        }
    }
    Ok(())
}

async fn dispatch_tray_cmd(
    orch:   &Arc<Mutex<Orchestrator>>,
    handle: &ksni::Handle<reclix_tray::ReclixTray>,
    cmd:    TrayCommand,
) {
    let result = match cmd {
        TrayCommand::PauseResume       => orch.lock().await.handle_pause_resume().await,
        TrayCommand::Finish            => orch.lock().await.handle_finish().await,
        TrayCommand::ToggleMic         => orch.lock().await.handle_toggle_mic().await,
        TrayCommand::ToggleSystemAudio => {
            warn!("system audio toggle not yet implemented (V0.2)");
            Ok(())
        }
        TrayCommand::OpenSettings => {
            info!("settings dialog not yet implemented (V0.2)");
            Ok(())
        }
    };
    if let Err(e) = result {
        error!("tray command error: {e:#}");
        crate::notify::show_error("Reclix error", &e.to_string());
    }
    let _ = handle.update(|_| true).await;
}

async fn dispatch_shortcut_event(
    orch:   &Arc<Mutex<Orchestrator>>,
    handle: &ksni::Handle<reclix_tray::ReclixTray>,
    ev:     ShortcutEvent,
) {
    let result = match ev {
        ShortcutEvent::Start       => orch.lock().await.handle_start().await,
        ShortcutEvent::PauseResume => orch.lock().await.handle_pause_resume().await,
        ShortcutEvent::Finish      => orch.lock().await.handle_finish().await,
    };
    if let Err(e) = result {
        error!("shortcut error: {e:#}");
        crate::notify::show_error("Reclix error", &e.to_string());
    }
    let _ = handle.update(|_| true).await;
}
