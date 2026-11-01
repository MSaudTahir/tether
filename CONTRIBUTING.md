# Contributing

How Tether is put together, and the rules for changing it. [README.md](README.md) covers
building; [ROADMAP.md](ROADMAP.md) covers what's done and what's next.

## How it's organized

- **All logic lives in the Rust core** (`core/`): adb, scrcpy, caching, settings, parsing. The apps
  are thin native UIs on top of it: SwiftUI on macOS, GTK4 + libadwaita on Linux, WinUI 3 on
  Windows. If two apps would need the same code, it belongs in the core.
- **Each app looks native to its platform** (Liquid Glass, libadwaita, Fluent), rather than one
  shared look.
- **The core reaches the apps through UniFFI**: Swift and C# bindings are generated at build time;
  the Linux app uses the core directly. Core calls block, so apps run them off the main thread.
- **On the phone, prefer adb alone.** When adb can't do something, extend the on-device helper
  (`helper/`, run with `app_process`, nothing installed) before reaching for Tether Buddy
  (`buddy/`, an installed companion). Buddy is for what only an installed app can do, and Tether
  must keep working without it.
- **Know the Android versions**: features that need a newer Android are disabled with a hint on
  older phones (`device_capabilities` in the core).

## Rules

- **Keep it simple.** Few, small modules; no new frameworks or heavy dependencies without a good
  reason. Match the style of the code around a change.
- **Keep the docs current in the same change**: [README.md](README.md) for anything a user sees
  (features, status, requirements, build steps, shortcuts, settings) and [ROADMAP.md](ROADMAP.md)
  for progress and plans. Don't claim a platform or feature works until it has been built and run.
- **Keep all three apps level.** A feature lands on macOS, Linux and Windows together, or the
  roadmap says what's missing where.
- **No secrets or personal details in the repo**: signing keys (`*.keystore`), certificates,
  passwords, machine names, addresses or emails. Keys for releases belong in CI secrets.
- **Commit small, describe why**, and keep `main` building.

## Testing

- Core logic gets unit tests next to it (`cargo test` in `core/`), especially parsers of adb and
  dumpsys output. Use real output from a phone in the test, with serials, addresses, names and
  message text replaced by made-up ones.
- `cargo run --example smoke -- <data dir>` in `core/` exercises the core against a connected phone.
- UI changes should be seen running, not just built:
  - macOS: `open --env TETHER_SECTION=Files macos/build/Tether.app` (any section name) opens on
    that section.
  - Linux (debug builds): `TETHER_SNAPSHOT=/tmp/shot.png` saves the window as an image after a few
    seconds, and `TETHER_SNAPSHOT_SECTION=files` picks the section.
  - Windows: CI launches the app and keeps a screenshot; crashes are written to
    `%LOCALAPPDATA%\Tether\crash.log` and printed in the CI log.
- Ask before changing settings or data on a real phone when testing.

## CI and releases

- Each platform has a workflow in `.github/workflows/` (`core`, `macos`, `linux`, `windows`) that
  runs when its files or the core change. Each can also be run by hand.
- Pushing a `v*` tag runs `release.yml`: it builds every app and creates a **draft** release, which
  is checked and then published by hand. Versions come from the tag; bump `core/Cargo.toml`,
  `linux/Cargo.toml`, `macos/Info.plist` and `windows/Tether.csproj` in the release commit too.
- Tether Buddy's signing key is kept outside the repo. Phones only update Buddy when it's signed
  with the same key, so keep a backup of it.
