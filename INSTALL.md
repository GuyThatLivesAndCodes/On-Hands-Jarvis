# Installing On-Hands Jarvis

Pre-built installers are produced by the **Release Installers** workflow
in CI. Each tagged release attaches them to the GitHub release page; in
the meantime you can grab the latest artifacts from any successful CI
run.

| Platform | File | Auto-start at login |
| --- | --- | --- |
| Debian / Ubuntu | `on-hands-jarvis_<version>_amd64.deb` | Yes (XDG autostart) |
| macOS 11+       | `On-Hands-Jarvis-<version>.dmg`        | Optional (one-click `Enable Autostart.command`) |
| Windows 10/11   | `on-hands-jarvis-<version>.msi`        | Yes (Startup-folder shortcut + HKCU\…\Run) |

## Linux (Debian / Ubuntu)

```bash
sudo apt install ./on-hands-jarvis_<version>_amd64.deb
```

The package installs the binary to `/usr/bin/on-hands-jarvis`, drops a
`.desktop` launcher into `/usr/share/applications/`, and enables
auto-start at login via `/etc/xdg/autostart/on-hands-jarvis.desktop`.

To disable auto-start without uninstalling, remove the autostart entry:

```bash
sudo rm /etc/xdg/autostart/on-hands-jarvis.desktop
```

## macOS

1. Open the `.dmg`.
2. Drag **On-Hands Jarvis.app** to the **Applications** folder.
3. (Optional) Double-click **Enable Autostart.command** to install the
   `LaunchAgent` so Jarvis launches at login. Re-run anytime to refresh
   it; remove with:
   ```bash
   launchctl unload ~/Library/LaunchAgents/com.onhands.jarvis.plist
   rm ~/Library/LaunchAgents/com.onhands.jarvis.plist
   ```

The first launch will trigger macOS prompts for microphone,
screen-recording, and accessibility permissions. Grant them in
**System Settings → Privacy & Security**.

## Windows

Run the `.msi`. The installer:

- Copies `on-hands-jarvis.exe` into `C:\Program Files\On-Hands Jarvis\`.
- Creates a Start Menu shortcut.
- Adds a Startup-folder shortcut and an
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` entry so Jarvis
  launches automatically at login.

To uninstall, use **Settings → Apps & features**.

## Building from source

See [`BUILD.md`](BUILD.md) for system dependencies and `cargo build`
instructions on each platform. To produce an installer locally:

```bash
# Linux (.deb)
cargo install cargo-deb
cargo deb

# macOS (.dmg)
cargo build --release
bash packaging/macos/bundle.sh

# Windows (.msi) — from a Developer PowerShell
cargo install cargo-wix
cargo wix
```
