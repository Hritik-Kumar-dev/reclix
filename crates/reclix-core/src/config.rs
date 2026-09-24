//! Reclix configuration — loaded from `~/.config/reclix/config.json`.
//! All fields are optional; missing keys fall back to defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Keyboard shortcuts (expressed as X11/GNOME shortcut strings).
/// On Wayland with the XDG GlobalShortcuts portal these are registered
/// programmatically; on unsupported compositors the user is directed to
/// add them as GNOME custom keybindings calling `reclix <cmd>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    /// Start a new recording.  Default: `<Super><Shift>r`
    pub start: String,
    /// Pause / resume the current recording.
    /// NOTE: bare `space` cannot be registered globally on Wayland without
    /// portal support.  Default uses `<Super><Shift>space` as a safe fallback;
    /// users may override this to `space` once they confirm their compositor
    /// supports it.
    pub pause_resume: String,
    /// Toggle focus mode.  Default: `<Alt>f`
    pub focus: String,
    /// Toggle camera overlay.  Default: `<Alt>c`
    pub camera: String,
    /// Toggle microphone.  Default: `<Alt>m`
    pub mic: String,
    /// Toggle system audio.  Default: `<Alt>s`
    pub system_audio: String,
    /// Finish recording.  Default: `Escape`
    pub finish: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            start:              "<Super><Shift>r".into(),
            pause_resume:       "<Super><Shift>space".into(),
            focus:              "<Alt>f".into(),
            camera:             "<Alt>c".into(),
            mic:                "<Alt>m".into(),
            system_audio:       "<Alt>s".into(),
            finish:             "Escape".into(),
        }
    }
}

/// Top-level Reclix configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclixConfig {
    /// Where to write final MP4 exports.
    /// Defaults to `~/Videos/Reclix/`.
    pub output_dir: Option<PathBuf>,

    /// Frame rate for capture.  Defaults to 30.
    pub fps: u32,

    /// Prefer VA-API hardware encoding.  Falls back to libx264 automatically.
    pub prefer_vaapi: bool,

    /// CRF/quality for libx264 fallback (0 = lossless, 23 = default, 51 = worst).
    pub x264_crf: u32,

    /// Target video bitrate string for VA-API (e.g. "4M").
    pub vaapi_bitrate: String,

    pub shortcuts: ShortcutConfig,

    /// Enable microphone by default when a new session starts.
    pub mic_enabled_by_default: bool,
}

impl Default for ReclixConfig {
    fn default() -> Self {
        Self {
            output_dir:             None,
            fps:                    30,
            prefer_vaapi:           true,
            x264_crf:               23,
            vaapi_bitrate:          "4M".into(),
            shortcuts:              ShortcutConfig::default(),
            mic_enabled_by_default: true,
        }
    }
}

impl ReclixConfig {
    /// Path to the config file: `~/.config/reclix/config.json`.
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("reclix").join("config.json"))
    }

    /// Load config from disk; return `Default` if the file is absent.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else { return Self::default() };
        if !path.exists() { return Self::default(); }
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("failed to read config at {}: {e}", path.display());
                return Self::default();
            }
        };
        match serde_json::from_str(&data) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("config parse error: {e}; using defaults");
                Self::default()
            }
        }
    }

    /// Persist config to disk; creates parent directories as needed.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| std::io::Error::other("no config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = ReclixConfig::default();
        assert_eq!(cfg.fps, 30);
        assert!(cfg.prefer_vaapi);
        assert_eq!(cfg.x264_crf, 23);
    }

    #[test]
    fn config_roundtrip() {
        let cfg = ReclixConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let loaded: ReclixConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.fps, cfg.fps);
        assert_eq!(loaded.shortcuts.start, cfg.shortcuts.start);
    }
}
