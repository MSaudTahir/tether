<p align="center"><img src="icon/icon.png" width="128" alt=""></p>

# MyDevice

A small desktop app for using your Android phone from your computer, built on
[scrcpy](https://github.com/Genymobile/scrcpy).

- **Mirror** the phone's screen in a window, and control it with your mouse and keyboard.
- **Open apps in their own windows**: pick an app from a grid of your phone's apps and it opens
  on a separate virtual display, at the size you choose (HD, Full HD, QHD, portrait or landscape).
  Needs Android 10+.
- **USB or Wi-Fi**: pair over Wi-Fi with a pairing code (Android 11+), or switch a USB-connected
  phone to Wi-Fi with one click.
- **Several phones** at once, with their name, model, Android version and battery.
- **Capture Mouse** for games: the mouse acts as a real mouse on the phone (relative movement,
  pointer capture). Press Left ⌥/Alt or ⌘ to release it.
- **Sets itself up**: downloads the latest scrcpy (with adb) on first run, checks for updates, and
  can use Android Studio's or your system's adb instead.

> [!NOTE]
> The macOS app is ready. Windows (WinUI 3) and Linux (GTK4) apps are planned; until then,
> [`python/`](python/) has an older cross-platform Qt version.

## Requirements

- **macOS 26** or later.
- An Android phone with **USB debugging** turned on (Settings → About phone → tap *Build number*
  7 times, then Developer options → USB debugging). Android 5+ for mirroring, Android 10+ for app
  windows.

## Install

Download the latest build from the [Releases](../../releases) page, unzip it and move
`MyDevice.app` to Applications.

The app isn't notarized yet, so macOS will block it the first time. Right-click the app and
choose **Open**, or run:

```sh
xattr -dr com.apple.quarantine /Applications/MyDevice.app
```

## Using it

1. On first launch, the setup wizard downloads scrcpy and picks an adb.
2. Connect your phone with USB and accept the *Allow USB debugging?* prompt on the phone. For Wi-Fi,
   use **Pair** (Developer options → Wireless debugging → *Pair device with pairing code*), or
   press **Go Wireless** while connected by USB.
3. Select the phone in the sidebar, then **Mirror Screen** (⇧⌘M) or click an app to open it in its
   own window. **Refresh** (⌘R) reloads the app list; it's cached until then.

### Tips

- **Capture Mouse** in app windows turns on the phone's *Force desktop mode* developer option so the
  mouse shows up on the app's display instead of the phone screen. You can turn it off again in
  Developer options.
- **Text Size** in the window-size menu changes the virtual display's density: *Smaller* fits more
  on screen.
- Some phones (e.g. Xiaomi/MIUI) need **USB debugging (Security settings)** turned on before the
  mouse and keyboard work.

## Building from source

You need [Rust](https://rustup.rs) (1.86+) and Xcode 26.

```sh
git clone <this repo>
cd mydevice
macos/build.sh                 # → macos/build/MyDevice.app
open macos/build/MyDevice.app
```

`build.sh` builds the Rust core as a static library, generates its Swift bindings with
[UniFFI](https://mozilla.github.io/uniffi-rs/), builds the app with SwiftPM, and puts together the
`.app` bundle (ad-hoc signed). The app's data (scrcpy, the app cache, settings) lives in
`~/Library/Application Support/app.mydevice.MyDevice`.

### Project layout

One shared core holds all the logic; each platform gets a thin native UI on top of it.

| Folder | What |
|---|---|
| [`core/`](core/) | Rust: adb, scrcpy download and updates, the app and icon cache, pairing, scrcpy windows, settings. |
| [`macos/`](macos/) | SwiftUI app with Liquid Glass. |
| [`helper/`](helper/) | A tiny program that runs on the phone (via `app_process`) to list apps with their icons. The core embeds the built `helper.dex`. |
| [`icon/`](icon/) | The app icon and the script that draws it (`swift icon/make_icon.swift icon/icon.png`). |
| [`python/`](python/) | The original Qt app, until the Windows and Linux apps exist. |

### Core

```sh
cd core
cargo test
cargo run --example smoke -- /tmp/mydevice-data   # runs everything against a connected phone
```

Core calls block (they run adb, scrcpy or downloads), so the UIs call them off the main thread.

### Helper

`helper/helper.dex` is checked in, so you only need this if you change `Helper.java`.
`helper/build.sh` rebuilds it; it needs a JDK and the Android SDK (`d8`).

## Thanks

Special thanks to [**Genymobile**](https://www.genymobile.com) (the makers of Genymotion) and
[Romain Vimont](https://github.com/rom1v) for [**scrcpy**](https://github.com/Genymobile/scrcpy).
All the mirroring, control and virtual display magic is scrcpy; this app is just a friendly front
end for it. If you find it useful, consider [supporting its author](https://github.com/sponsors/rom1v)
too.

scrcpy is licensed under the Apache License 2.0. MyDevice downloads official scrcpy releases from
GitHub (verifying their SHA-256 checksums) rather than redistributing it.

Android is a trademark of Google LLC. The Android robot is reproduced or modified from work created
and shared by Google and used according to terms described in the Creative Commons 3.0 Attribution
License. This project isn't affiliated with Google or Genymobile.
