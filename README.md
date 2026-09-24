# Reclix — lightweight Linux-native screen recorder

> **"Invisible when recording. Powerful when needed."**

Keyboard-first screen recorder for Ubuntu/GNOME (Wayland-first, X11 fallback). Not an OBS clone, not a screenshot tool.

## V0.1 Status — Implemented

| Feature | Status |
|---|---|
| Cargo workspace (7 crates) | ✅ |
| State machine `IDLE→STARTING→RECORDING⇄PAUSED→FINISHING→COMPLETED` (17 tests) | ✅ |
| Session model (segments, JSON metadata, `~/Videos/Reclix/`) | ✅ |
| XDG ScreenCast portal + PipeWire capture (full-screen) | ✅ |
| Microphone via PipeWire | ✅ |
| FFmpeg encoding `VA-API → libx264` fallback, segment concat → MP4 | ✅ |
| D-Bus service `org.reclix.Service1` + `systemd --user` + auto-activation | ✅ |
| `reclix` CLI (`start | pause | resume | finish | status | mic`) | ✅ |
| Tray indicator (ksni StatusNotifierItem, `media-record` ↔ `🔴 mm:ss` ↔ `⏸ mm:ss`) | ✅ |
| Global shortcut `Super+Shift+R` via XDG GlobalShortcuts portal | ✅ |
| Notifications via `notify-send` | ✅ |

## Workspace Layout

```
reclix/
├─ crates/
│  ├─ reclix-core/     # state machine, session, config, dbus types (no system deps)
│  ├─ reclix-capture/  # portal + PipeWire video (feature = pipewire-capture)
│  ├─ reclix-audio/    # PipeWire mic (feature = pipewire-audio)
│  ├─ reclix-encode/   # FFmpeg VA-API/x264 pipeline
│  ├─ reclix-service/  # D-Bus + orchestrator + shortcuts + systemd
│  ├─ reclix-tray/     # ksni tray
│  └─ reclix-cli/      # reclix binary
├─ data/
│  ├─ reclix.service                 # systemd user unit
│  ├─ org.reclix.Service1.service    # D-Bus activation
│  ├─ reclix.desktop                 # .desktop (NoDisplay)
│  └─ install.sh                     # user install script
└─ docs/
```

## Requirements

- Rust ≥1.75, `ffmpeg` on PATH, PipeWire running
- For full build (capture + mic): `libpipewire-0.3-dev` + `libspa-0.2-dev`
- Ubuntu 22.04+/GNOME 42+; GlobalShortcuts portal needs `xdg-desktop-portal ≥1.12`, GNOME Shell ≥44

## Build

```bash
# Without system deps (CI, pure-Rust check + tests):
cargo check --no-default-features
cargo test  --no-default-features
cargo clippy --no-default-features

# Full build (on Ubuntu with PipeWire headers):
sudo apt install libpipewire-0.3-dev libspa-0.2-dev
cargo build --release --features reclix-service/full
# or per-crate: cargo build -p reclix-service --features full
```

Hardware encoding is probed at runtime: `h264_vaapi` if `/dev/dri/renderD128` + ffmpeg vaapi works, else `libx264 veryfast crf23`.

## Install (user)

```bash
cargo install --path crates/reclix-service --features full
cargo install --path crates/reclix-cli
bash data/install.sh          # installs systemd unit, D-Bus service, .desktop
systemctl --user start reclix.service
reclix status
```

Outputs go to `~/Videos/Reclix/reclix_YYYYMMDD_HHMMSS.mp4`; raw segments + `session.json` live in the session dir until exported.

## Usage

```bash
reclix start          # Super+Shift+R
reclix pause          # Super+Shift+Space (or tray → Pause)
reclix resume         # same toggle
reclix finish         # Esc or tray → Finish & Export
reclix status
reclix mic            # toggle mute
reclix camera         # V0.3+
reclix focus          # V0.4+
```

Tray menu: Pause/Resume, Finish, Mic toggle, System Audio (V0.2, disabled), shortcut cheat-sheet, Settings (V0.2).

Config: `~/.config/reclix/config.json` (fps, vaapi bitrate, x264 crf, shortcuts, mic default). See `reclix-core/src/config.rs`.

## Wayland / Shortcuts — Honest Notes

- **Bare `Space` cannot be a global shortcut on Wayland.** Compositor owns the keyboard when no window has focus; GNOME won't forward printable keys. Default is `Super+Shift+Space`. XDG GlobalShortcuts portal is the only Wayland-safe mechanism.
- If the portal is unavailable (older portal/GNOME, X11), the service logs `gsettings` instructions and you can bind keys manually:
  ```bash
  BASE=/org/gnome/settings-daemon/plugins/media-keys
  CB=$BASE/custom-keybindings
  gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['$CB/reclix0/', '$CB/reclix1/']"
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ name 'Reclix Start'
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ command 'reclix start'
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix0/ binding '<Super><Shift>r'
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ name 'Reclix Pause'
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ command 'reclix pause'
  gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$CB/reclix1/ binding '<Super><Shift>space'
  ```
- `XGrab`-based fallback for X11 is planned for V0.2.

## Architecture

```
portal (ashpd) ─fd/node_id─► PipeWire thread ─RawFrame─► mpsc ─► SegmentEncoder (ffmpeg via pipe)
                                                    AudioFrame ─► (same ffmpeg, second pipe)
Systemd --user ─► reclix-service (tokio) ─┬─ D-Bus org.reclix.Service1 ─► reclix CLI
                                          ├─ ksni tray (async, Handle::update)
                                          └─ GlobalShortcuts portal ─► ShortcutEvent
State: Orchestrator owns Session + StateMachine + CaptureHandle + MicHandle + SegmentEncoder.
Pause = close segment encoder + capture.pause() (discard frames); resume = new segment.
Finish = concat demuxer (ffmpeg -f concat -c copy -movflags +faststart) → MP4.
Idle CPU ≈ 0 (tokio select! blocks).
```

No frames are processed in Rust — orchestrate the media stack.

## Roadmap

- **V0.2** region/window capture, system audio, timer, configurable shortcuts
- **V0.3** webcam overlay
- **V0.4** focus mode (cursor Metadata, 2× zoom)
- **V0.5** session history / segment management
- **V1.0** lightweight editor, .deb/Flatpak

## License

GPL-3.0-or-later
