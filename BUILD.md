# Building On-Hands Jarvis

A Rust + `eframe`/`egui` desktop application. The build is plain
`cargo build`; what follows is just the system libraries each platform
needs for the dependencies that wrap C libs.

## Linux

Tested on Ubuntu 24.04. Install once:

```bash
sudo apt install \
    build-essential pkg-config \
    libasound2-dev \
    libdbus-1-dev \
    libxdo-dev \
    libxkbcommon-x11-0 \
    libfontconfig-dev libx11-dev libxcb1-dev
```

Then:

```bash
cargo build --release
./target/release/on-hands-jarvis
```

`libasound2-dev` is needed by `cpal` (microphone capture), `libxdo-dev`
by `enigo` (mouse/keyboard simulation), `libdbus-1-dev` by `arboard`
(clipboard), and `libxkbcommon-x11-0` by `winit` at runtime.

## macOS

```bash
cargo build --release
```

You may need to grant the binary microphone, screen-recording, and
accessibility permissions the first time you run it.

## Windows

```powershell
cargo build --release
```

No additional system libraries are needed.

## Configuration

On first run the setup wizard records ten samples of your wake word and
optionally takes your xAI / Grok API key. The configuration is stored
under your platform's standard config directory (e.g.
`~/.config/Jarvis/config.json` on Linux).
