# Roadmap

Where Tether is and where it's going. Update this file when something lands or plans change
(see [CONTRIBUTING.md](CONTRIBUTING.md)).

## Released

### 1.0.0

- macOS (SwiftUI, Liquid Glass), Linux (GTK4 + libadwaita) and Windows (WinUI 3) apps on one Rust
  core.
- Mirroring (one window per phone) and apps in their own resizable windows (virtual displays).
- Sections: Overview (status, controls, details, clipboard), Apps (with app actions),
  Notifications (cards; Tether Buddy runs their actions), Photos, Files.
- Setup wizard, scrcpy download and updates, USB and Wi-Fi (pairing, Go Wireless, reconnect).
- Releases built by GitHub Actions: `.dmg`, Windows installer and zip (x64, ARM64), Linux `.deb`,
  `.rpm` and `.tar.gz` (x86_64, ARM64).

## Known limitations

- Builds aren't signed: macOS needs right-click → Open the first time; Windows SmartScreen warns.
  Signing needs an Apple Developer ID and a Windows code-signing certificate.
- The macOS build is Apple Silicon only.
- Linux ARM64 packages aren't tested, and scrcpy has no prebuilt Linux ARM64 build (users install
  scrcpy 3.0+ themselves).
- Resizable app windows aren't Retina-sharp on macOS: scrcpy sizes a flex display's window in
  points there.
- Some phones (e.g. Xiaomi) don't let adb read the clipboard in the background, so "Copy Phone
  Clipboard" can't work on them.
- A notification's action can't open in an app window (it's started as the app that posted it,
  which can't use scrcpy's display), so notifications open in the mirror.
- The Windows download is ~70 MB: .NET and WinUI ship with the app. Trimming .NET breaks WinUI's
  interop.

## Next

Roughly in order:

1. **Unlock with Touch ID / Windows Hello**: keep the phone's PIN in the system keychain, protected
   by biometrics, and type it over adb to unlock the phone.
2. **Universal macOS build** (Intel and Apple Silicon).
3. **Tether's own mirror view**: implement scrcpy's client side (video decoding, input) in the core
   so the phone can show in native windows (rounded corners, glass/Mica) or inside the main window.

## Ideas

- Screen recording (scrcpy `--record`).
- The phone's camera as a webcam (scrcpy, Android 12+; a real webcam device on Linux).
- Gamepad passthrough (scrcpy `--gamepad`).
- Favorite or recent apps at the top of the grid.
- A menu bar / tray icon for quick Mirror and Screenshot.
- Finding phones on the network that have Wireless debugging on.
- Signed and notarized builds.
