# Reclix — Lightweight Linux-native Screen Recorder

> **'Invisible when recording. Powerful when needed.'**

**Reclix** is a keyboard-first screen recorder tailored for Ubuntu and GNOME environments, prioritizing Wayland compatibility with X11 fallback. Unlike traditional screenshot tools or OBS clones, it offers seamless recording functionality.

## Features Overview

### Implemented Features

| Feature | Status |
|---|---|
| Cargo workspace (7 crates) | ✅ |
| State machine  (17 tests) | ✅ |
| Session model (segments, JSON metadata, stored in ) | ✅ |
| XDG ScreenCast protocol integration + PipeWire full-screen capture | ✅ |
| Microphone capture via PipeWire | ✅ |
| FFmpeg encoding with  fallback to , segment concatenation to MP4 | ✅ |
| D-Bus service , integrated with  for auto-activation | ✅ |
| Command Line Interface ( CLI with commands: start, pause, resume, finish, status, mic) | ✅ |
| Tray indicator displaying recording status and duration | ✅ |
| Global shortcut  via XDG GlobalShortcuts portal | ✅ |
| Notifications via  | ✅ |

## Workspace Structure

The workspace contains several crates organized as follows:


## System Requirements

To build and run **Reclix**, ensure you meet the following requirements:
- Rust version ≥1.75
-  must be available in your system's PATH
- PipeWire service should be running
- For a complete build including both video and audio capture, install the following dependencies:
  - 
  - 
- Compatible with Ubuntu 22.04+ and GNOME 42+; Ensure  is version ≥1.12 and GNOME Shell is version ≥44.

## Build Instructions

### Without System Dependencies
To run tests and checks without any system dependencies:


### Full Build Instructions
To build **Reclix** on Ubuntu with all required PipeWire headers:
Reading package lists...
Building dependency tree...
Reading state information...

**Hardware Encoding:** During runtime, hardware encoding is determined; it will utilize  if available and functional, otherwise defaults to .

## User Installation
To install **Reclix**:
==> Installing binaries to /home/user/.cargo/bin
==> Installing systemd user unit

### Output Path
Recording output is saved to the following directory:
. Raw segment files and  are stored in the session directory until exported.

## Usage Notes
The following commands can be used in the terminal:


### Tray Menu
Access the tray menu for options to Pause/Resume, Finish, Mic toggle, and view the Settings (available in V0.2).

**Configuration File:** You can customize settings in , defining options such as fps, VAAPI bitrate, x264 crf, shortcuts, and default microphone settings. More details can be found in .

## Notes on Wayland and Shortcuts
- **Global Shortcuts:** The  key cannot be designated as a global shortcut under Wayland due to compositor restrictions. The default global shortcut is . The XDG GlobalShortcuts portal is the recommended mechanism for Wayland environments.
- In cases where the portal is not available (e.g., legacy versions or X11), manual key bindings can be configured using  as illustrated below:

- A fallback method using  for X11 compatibility is planned for a future update (V0.2).

## Architecture Overview



**Performance Note:** No frames are processed in Rust as this component primarily orchestrates the media processing stack.

## Future Roadmap
- **V0.2**: Introduce region/window capture, system audio handling, timer functionality, and adjustable shortcuts.
- **V0.3**: Add webcam overlay support.
- **V0.4**: Implement focus mode with cursor metadata and 2× zoom capability.
- **V0.5**: Enable session history and segment management functionalities.
- **V1.0**: Release a lightweight editor along with packages for .deb and Flatpak.

## Licensing
**Reclix** is licensed under GPL-3.0-or-later.

