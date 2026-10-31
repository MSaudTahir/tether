# MyDevice

A small desktop app (macOS, Windows, Linux) for viewing Android devices, built on
[scrcpy](https://github.com/Genymobile/scrcpy) and Qt (PySide6).

- Lists USB and Wi-Fi devices live, with name, model, Android version, battery and a screenshot.
- **Mirror screen**: mirrors the device display in a scrcpy window.
- **App windows**: double-click an app to open it on its own virtual display
  (`--new-display`, Android 10+) at a resolution you pick. Each app gets its own window.
- Downloads the latest scrcpy for your platform on first run (checksum-verified) and offers updates.
- Uses Android Studio's adb, the system adb or scrcpy's bundled adb (Auto prefers Studio's so the
  adb server Android Studio is using isn't restarted). Choose via **ADB…**.
- Wireless: pair with a pairing code, connect by address, or switch a USB device to Wi-Fi.

## Run

```sh
python3 -m venv .venv
.venv/bin/pip install -e .
.venv/bin/mydevice        # or: .venv/bin/python -m mydevice
```

scrcpy is stored in the app data folder (e.g. `~/Library/Application Support/mydevice/MyDevice/scrcpy`).

## Code

| File | What |
|---|---|
| `mydevice/toolchain.py` | finding adb, downloading/updating scrcpy |
| `mydevice/adb.py` | adb/scrcpy commands and output parsing (blocking) |
| `mydevice/ui.py` | the window; blocking calls run in the background via `Bg` |

## Next

- Real app icons and wallpaper (needs a small on-device helper run via `app_process`)
- QR-code wireless pairing (mDNS)
- Packaging (PyInstaller / briefcase)
