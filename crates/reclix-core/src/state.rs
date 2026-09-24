//! Recording state machine.
//!
//! ```text
//! IDLE ──start──► STARTING ──ready──► RECORDING ──pause──► PAUSED
//!                                          ▲                  │
//!                                          └────resume─────────┘
//!                                          │
//!                                       finish
//!                                          │
//!                                          ▼
//!                                      FINISHING ──done──► COMPLETED
//! ```
//!
//! Any state can transition to IDLE via `reset()` (used after COMPLETED or on
//! hard error recovery).

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{ReclixError, Result};

/// The five top-level states of a Reclix recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordingState {
    /// No active session; service is waiting.
    Idle,
    /// Portal negotiation and PipeWire stream setup in progress.
    Starting,
    /// Actively capturing frames and audio.
    Recording,
    /// Capture paused; current segment closed; timer frozen.
    Paused,
    /// Concatenating segments into final MP4.
    Finishing,
    /// Export complete; file is on disk.
    Completed,
}

impl fmt::Display for RecordingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RecordingState::Idle      => "IDLE",
            RecordingState::Starting  => "STARTING",
            RecordingState::Recording => "RECORDING",
            RecordingState::Paused    => "PAUSED",
            RecordingState::Finishing => "FINISHING",
            RecordingState::Completed => "COMPLETED",
        };
        write!(f, "{s}")
    }
}

/// Events that drive the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEvent {
    /// User pressed Start.
    Start,
    /// Portal + PipeWire stream are ready; begin actual capture.
    Ready,
    /// User pressed Pause.
    Pause,
    /// User pressed Resume.
    Resume,
    /// User pressed Finish.
    Finish,
    /// Encoding pipeline finished successfully.
    Done,
    /// Reset to IDLE (after COMPLETED or error recovery).
    Reset,
}

/// The state machine itself.  This is `Send + Sync` and contains no async
/// primitives — it is driven by the service layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    state: RecordingState,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self { state: RecordingState::Idle }
    }
}

impl StateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current state.
    pub fn state(&self) -> RecordingState {
        self.state
    }

    /// Attempt a transition driven by `event`.  Returns the new state on
    /// success, or [`ReclixError::InvalidTransition`] on an illegal event.
    pub fn transition(&mut self, event: StateEvent) -> Result<RecordingState> {
        use RecordingState::*;
        use StateEvent::*;

        let next = match (self.state, event) {
            (Idle,      Start)  => Starting,
            (Starting,  Ready)  => Recording,
            (Recording, Pause)  => Paused,
            (Paused,    Resume) => Recording,
            (Recording, Finish) => Finishing,
            (Paused,    Finish) => Finishing,
            (Finishing, Done)   => Completed,
            // Reset is allowed from any state.
            (_,         Reset)  => Idle,
            (from, ev) => {
                return Err(ReclixError::InvalidTransition {
                    from: from.to_string(),
                    to:   format!("{ev:?}"),
                });
            }
        };

        tracing::info!(from = %self.state, to = %next, "state transition");
        self.state = next;
        Ok(next)
    }

    /// Returns `true` if currently capturing (RECORDING state).
    pub fn is_recording(&self) -> bool {
        self.state == RecordingState::Recording
    }

    /// Returns `true` if a session is active (not IDLE or COMPLETED).
    pub fn is_active(&self) -> bool {
        !matches!(self.state, RecordingState::Idle | RecordingState::Completed)
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sm() -> StateMachine {
        StateMachine::new()
    }

    #[test]
    fn happy_path_no_pause() {
        let mut m = sm();
        assert_eq!(m.state(), RecordingState::Idle);
        m.transition(StateEvent::Start).unwrap();
        assert_eq!(m.state(), RecordingState::Starting);
        m.transition(StateEvent::Ready).unwrap();
        assert_eq!(m.state(), RecordingState::Recording);
        m.transition(StateEvent::Finish).unwrap();
        assert_eq!(m.state(), RecordingState::Finishing);
        m.transition(StateEvent::Done).unwrap();
        assert_eq!(m.state(), RecordingState::Completed);
    }

    #[test]
    fn happy_path_with_pause_resume() {
        let mut m = sm();
        m.transition(StateEvent::Start).unwrap();
        m.transition(StateEvent::Ready).unwrap();
        m.transition(StateEvent::Pause).unwrap();
        assert_eq!(m.state(), RecordingState::Paused);
        m.transition(StateEvent::Resume).unwrap();
        assert_eq!(m.state(), RecordingState::Recording);
        // pause again, then finish from paused
        m.transition(StateEvent::Pause).unwrap();
        m.transition(StateEvent::Finish).unwrap();
        assert_eq!(m.state(), RecordingState::Finishing);
        m.transition(StateEvent::Done).unwrap();
        assert_eq!(m.state(), RecordingState::Completed);
    }

    #[test]
    fn reset_from_any_state() {
        for initial_event in [StateEvent::Start, StateEvent::Ready] {
            let mut m = sm();
            m.transition(StateEvent::Start).unwrap();
            if initial_event == StateEvent::Ready {
                m.transition(StateEvent::Ready).unwrap();
            }
            m.transition(StateEvent::Reset).unwrap();
            assert_eq!(m.state(), RecordingState::Idle);
        }
    }

    #[test]
    fn invalid_transition_is_error() {
        let mut m = sm();
        // Can't pause from IDLE
        let err = m.transition(StateEvent::Pause).unwrap_err();
        assert!(matches!(err, ReclixError::InvalidTransition { .. }));
        // State must be unchanged
        assert_eq!(m.state(), RecordingState::Idle);
    }

    #[test]
    fn cannot_finish_from_idle() {
        let mut m = sm();
        assert!(m.transition(StateEvent::Finish).is_err());
    }

    #[test]
    fn cannot_resume_when_not_paused() {
        let mut m = sm();
        m.transition(StateEvent::Start).unwrap();
        m.transition(StateEvent::Ready).unwrap();
        // Not paused yet — resume should fail
        assert!(m.transition(StateEvent::Resume).is_err());
        assert_eq!(m.state(), RecordingState::Recording);
    }

    #[test]
    fn is_recording_flag() {
        let mut m = sm();
        assert!(!m.is_recording());
        m.transition(StateEvent::Start).unwrap();
        assert!(!m.is_recording());
        m.transition(StateEvent::Ready).unwrap();
        assert!(m.is_recording());
        m.transition(StateEvent::Pause).unwrap();
        assert!(!m.is_recording());
    }
}
