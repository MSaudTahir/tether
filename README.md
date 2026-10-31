# MyDevice

A desktop app for viewing Android devices, built on [scrcpy](https://github.com/Genymobile/scrcpy):
mirror the screen, or open individual apps in their own windows (virtual displays, Android 10+).

One shared logic core, with a native UI per platform:

| Folder | What | Status |
|---|---|---|
| `core/` | Rust: adb, scrcpy download/update, app + icon cache, pairing, scrcpy sessions, config. Exposed to the UIs with [UniFFI](https://mozilla.github.io/uniffi-rs/). | ✅ |
| `macos/` | SwiftUI app with Liquid Glass (macOS 26+) | ✅ |
| `windows/` | WinUI 3 (C#) | planned |
| `linux/` | GTK4 / libadwaita (Rust, uses the core directly) | planned |
| `helper/` | Tiny on-device program (`app_process`) that lists apps with icons; the core embeds `helper.dex` | ✅ |
| `python/` | The original Qt app; kept until the Windows and Linux UIs replace it | legacy |

## macOS

Needs Xcode and Rust.

```sh
macos/build.sh          # → macos/build/MyDevice.app
open macos/build/MyDevice.app
```

`build.sh` builds the core as a static library, generates its Swift bindings, builds the app with
SwiftPM and bundles it. Data (scrcpy, the app cache, config) lives in
`~/Library/Application Support/app.mydevice.MyDevice`.

## Core

```sh
cd core
cargo test
cargo run --example smoke -- /tmp/mydevice-data   # exercises everything against a connected phone
```

All core calls block (they run adb/scrcpy or download), so UIs call them off the main thread.

## Helper

`helper/build.sh` rebuilds `helper/helper.dex` from `Helper.java` (needs a JDK and the Android SDK).
The core embeds it at compile time.
