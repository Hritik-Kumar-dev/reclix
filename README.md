# Reclix: Lightweight Linux-Native Screen Recorder

> **"Invisible when recording. Powerful when needed."**

Reclix is a keyboard-first screen recorder designed for Ubuntu/GNOME, operating primarily on Wayland with X11 as a fallback. This tool is not an OBS clone nor a mere screenshot utility.

## V0.1 Status: Implemented Features

The following features have been successfully implemented:

| Feature | Status |
|---|---|
| Cargo workspace (7 crates) | ✅ |
| State machine:  (17 tests) | ✅ |
| Session model with segments, JSON metadata, stored in  | ✅ |
| XDG ScreenCast portal with PipeWire capture for full-screen | ✅ |
| Microphone support through PipeWire | ✅ |
| FFmpeg encoding with fallback: , segment concatenation to MP4 | ✅ |
| D-Bus service  with  and auto-activation | ✅ |
| CLI commands available:  | ✅ |
| Tray indicator featuring , time elapsed display | ✅ |
| Global shortcut configured:  via XDG GlobalShortcuts portal | ✅ |
| Notification support through  | ✅ |

## Workspace Layout



## Requirements

- Rust version ≥1.75
-  in your PATH
- PipeWire must be running

For a full build (capture + mic), install:
- 
- 

Compatibility:
- Ubuntu 22.04+/GNOME 42+
- GlobalShortcuts portal requires , GNOME Shell ≥44

## Build Instructions

To compile without system dependencies (for CI purposes):


For a full build on Ubuntu with PipeWire headers:
Reading package lists...
Building dependency tree...
Reading state information...

### Installation (User)

To install Reclix:
==> Installing binaries to /home/user/.cargo/bin
==> Installing systemd user unit

Video outputs are saved to , with raw segments and  stored in their respective session directory until exported.

## Usage

Basic commands include:


The tray menu allows for Pause/Resume, Finish, and Mic toggle options, as well as system audio settings (V0.2), shortcut cheat-sheet, and more.

Configuration settings are stored in , and include options for fps, video bitrates, shortcuts, and mic defaults. Check  for more details.

## Wayland Shortcuts: Important Notes

- **Space can't be a global shortcut on Wayland.** The compositor takes control when no window is focused, preventing GNOME from forwarding printable keys. The default shortcut is . Use the XDG GlobalShortcuts portal for Wayland.
- If the portal is unavailable (e.g., older versions, X11), you can manually bind keys:
  
- A fallback using  for X11 is planned for V0.2.

## Architecture Overview



No frames are processed in Rust; this system orchestrates the media stack seamlessly.

## Future Roadmap

Upcoming features to anticipate:
- **V0.2:** Window/region capture, system audio integration, timer, configurable shortcuts
- **V0.3:** Webcam overlay capabilities
- **V0.4:** Focus mode with cursor metadata and zoom functionality
- **V0.5:** Session history and segment management
- **V1.0:** Introduction of a lightweight editor, .deb/Flatpak packaging

## License Information

This project is licensed under the GPL-3.0-or-later license.
