# Reclix — Lightweight Linux-Native Screen Recorder

> **"Invisible when recording. Powerful when needed."**

Reclix is a keyboard-first screen recorder designed for Ubuntu/GNOME (Wayland-first, X11 fallback). It is not just an OBS clone or a screenshot tool.

## Version 0.1 Status — Features Implemented

| Feature | Status |
|---|---|
| Cargo workspace (7 crates) | ✅ |
| State machine (IDLE→STARTING→RECORDING⇄PAUSED→FINISHING→COMPLETED) | ✅ |
| Session model (segments, JSON metadata, located at ) | ✅ |
| XDG ScreenCast portal + PipeWire capture (full-screen) | ✅ |
| Microphone via PipeWire | ✅ |
| FFmpeg encoding (VA-API → libx264 fallback, segment concatenation to MP4) | ✅ |
| D-Bus service (), systemd --user, and auto-activation | ✅ |
|  CLI commands () | ✅ |
| Tray indicator (ksni StatusNotifierItem, toggling between  and ) | ✅ |
| Global shortcut () via XDG GlobalShortcuts portal | ✅ |
| Notifications via  | ✅ |

## Workspace Layout



## Requirements

- Rust version ≥1.75,  in your PATH, and PipeWire must be running.
- For a complete build (including capture and microphone):  and 
- Recommended on Ubuntu 22.04+/GNOME 42+; the GlobalShortcuts portal requires  and GNOME Shell ≥44.

## Build Instructions

Reading package lists...
Building dependency tree...
Reading state information...

Hardware encoding is determined at runtime:  will be used if  is available and ffmpeg VAAPI functions properly; otherwise,  will be used.

## User Installation Steps

==> Installing binaries to /home/user/.cargo/bin
==> Installing systemd user unit

Output files will be saved to . Raw segments and  will remain in the session directory until exported.

## Usage Instructions



The tray menu includes options to pause/resume, finish, toggle microphone, control system audio (available in version 0.2, currently disabled), a shortcut cheat sheet, and settings (also in version 0.2).

Configuration file can be found at , where you can adjust fps, VAAPI bitrate, x264 crf, shortcuts, and microphone defaults. For more details, refer to .

## Wayland and Shortcuts — Notes

- **Bare  cannot function as a global shortcut on Wayland.** The compositor takes control of the keyboard when no window is focused, and GNOME will not forward printable keys. The default key binding is . The XDG GlobalShortcuts portal is the only Wayland-safe method.
- If the portal is unavailable (due to older portal/GNOME versions or X11), the service will log instructions to bind keys manually:
  
- A fallback using  is planned for version 0.2.

## Architecture Overview



No frames are processed using Rust — it orchestrates the media stack.

## Roadmap

- **Version 0.2:** Add region/window capture, system audio support, timer functionality, and configurable shortcuts.
- **Version 0.3:** Introduce webcam overlay support.
- **Version 0.4:** Implement focus mode (cursor metadata, 2× zoom).
- **Version 0.5:** Enhance session history and segment management functionality.
- **Version 1.0:** Launch a lightweight editor and provide .deb/Flatpak packaging options.

## License

This project is licensed under the GPL-3.0-or-later license.

