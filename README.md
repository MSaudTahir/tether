# MyDevice

A small desktop app (macOS, Windows, Linux) for viewing Android devices, built on
[scrcpy](https://github.com/Genymobile/scrcpy) and Qt (PySide6).

- Lists USB and Wi-Fi devices live, with name, model, Android version, battery and a screenshot.
- **Mirror screen**: mirrors the device display in a scrcpy window.
- **App windows**: double-click an app to open it on its own virtual display
  (`--new-display`, Android 10+) at a resolution you pick. Each app gets its own window.
- A first-run setup wizard (re-open it via **Setup…**):
  1. downloads the latest scrcpy for your platform (checksum-verified), or uses your own;
  2. picks adb: Android Studio's, the system one or scrcpy's bundled one. Auto prefers Studio's so
     the adb server Android Studio is using isn't restarted;
  3. walks you through enabling USB debugging and waits for the phone.
- Offers scrcpy updates in the status bar.
- Wireless: pair with a pairing code, connect by address, or switch a USB device to Wi-Fi.

## Run

```sh
python3 -m venv .venv
.venv/bin/pip install -e .
.venv/bin/mydevice        # or: .venv/bin/python -m mydevice
```

scrcpy is stored in the app data folder (e.g. `~/Library/Application Support/mydevice/MyDevice/scrcpy`).

## Standalone executables

```sh
.venv/bin/pip install -e ".[build]"
.venv/bin/python build.py
```

This builds for the OS you run it on: `dist/MyDevice-macos-<arch>.zip` (a .app), `MyDevice-windows-<arch>.exe`
or `MyDevice-linux-<arch>.tar.gz` (a single binary). Pushing a `v*` tag builds all of them on GitHub Actions
and attaches them to a GitHub release.

The builds aren't code-signed yet: macOS users need to right-click → Open the first time, and Windows
SmartScreen shows "More info → Run anyway". On Linux, Qt needs `libxcb-cursor0` (or your distro's equivalent).

## Code

| File | What |
|---|---|
| `mydevice/toolchain.py` | finding adb, downloading/updating scrcpy |
| `mydevice/adb.py` | adb/scrcpy commands and output parsing (blocking) |
| `mydevice/wizard.py` | first-run setup |
| `mydevice/ui.py` | the main window |
| `mydevice/tasks.py` | `Bg`: runs blocking calls off the UI thread |
| `build.py` | PyInstaller build for the current platform |

## Next

- Real app icons and wallpaper (needs a small on-device helper run via `app_process`)
- QR-code wireless pairing (mDNS)
- Code signing / notarization, app icon
