//! D-Bus interface `org.reclix.Service1`.
//!
//! parking_lot::Mutex guards are NOT Send, so we must never hold them across
//! an `.await` point.  Every handler pattern is:
//!   1. Lock, call the synchronous parts, clone what we need, drop the guard.
//!   2. Drive the async work with the lock *released*.
//!
//! For Orchestrator methods that are `async` themselves the simplest pattern
//! is to wrap the entire async call in a `tokio::task::spawn` or use an
//! `Arc<tokio::sync::Mutex>` (which IS Send).  We use `tokio::sync::Mutex`
//! for the orchestrator so we can hold it across awaits safely.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, info};
use zbus::{interface, object_server::SignalEmitter};

use reclix_core::dbus_types::ServiceCommand;

use crate::orchestrator::Orchestrator;

pub const SERVICE_NAME: &str = "org.reclix.Service1";
pub const OBJECT_PATH: &str  = "/org/reclix/Service1";

pub struct ReclixDbusService {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
}

impl ReclixDbusService {
    async fn status_json(&self) -> String {
        let status = self.orchestrator.lock().await.status();
        serde_json::to_string(&status).unwrap_or_else(|_| "{}".into())
    }
}

#[interface(name = "org.reclix.Service1")]
impl ReclixDbusService {
    async fn start(
        &mut self,
        #[zbus(signal_context)] ctx: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        info!("D-Bus: Start()");
        self.orchestrator.lock().await
            .handle_start().await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Self::state_changed(&ctx, &self.status_json().await).await.ok();
        Ok(())
    }

    async fn pause_resume(
        &mut self,
        #[zbus(signal_context)] ctx: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        info!("D-Bus: PauseResume()");
        self.orchestrator.lock().await
            .handle_pause_resume().await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Self::state_changed(&ctx, &self.status_json().await).await.ok();
        Ok(())
    }

    async fn finish(
        &mut self,
        #[zbus(signal_context)] ctx: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        info!("D-Bus: Finish()");
        self.orchestrator.lock().await
            .handle_finish().await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Self::state_changed(&ctx, &self.status_json().await).await.ok();
        Ok(())
    }

    async fn toggle_mic(
        &mut self,
        #[zbus(signal_context)] ctx: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        info!("D-Bus: ToggleMic()");
        self.orchestrator.lock().await
            .handle_toggle_mic().await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Self::state_changed(&ctx, &self.status_json().await).await.ok();
        Ok(())
    }

    async fn get_status(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus: GetStatus()");
        Ok(self.status_json().await)
    }

    async fn send_command(
        &mut self,
        command_json: String,
        #[zbus(signal_context)] ctx: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let cmd: ServiceCommand = serde_json::from_str(&command_json)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        debug!("D-Bus: SendCommand({cmd:?})");

        {
            let mut orch = self.orchestrator.lock().await;
            let result = match cmd {
                ServiceCommand::Start             => orch.handle_start().await,
                ServiceCommand::PauseResume       => orch.handle_pause_resume().await,
                ServiceCommand::Finish            => orch.handle_finish().await,
                ServiceCommand::ToggleMic         => orch.handle_toggle_mic().await,
                ServiceCommand::ToggleCamera      => Ok(()),
                ServiceCommand::ToggleFocus       => Ok(()),
                ServiceCommand::ToggleSystemAudio => Ok(()),
            };
            result.map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        } // lock released here

        Self::state_changed(&ctx, &self.status_json().await).await.ok();
        Ok(())
    }

    #[zbus(signal)]
    async fn state_changed(ctx: &SignalEmitter<'_>, status_json: &str) -> zbus::Result<()>;
}
