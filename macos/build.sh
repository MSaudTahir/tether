#!/bin/sh
# Builds the Rust core, generates its Swift bindings, builds the app and bundles build/Tether.app.
set -e
cd "$(dirname "$0")"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
CONFIG="${CONFIG:-release}"
export MACOSX_DEPLOYMENT_TARGET=26.0  # match Package.swift, so the Rust objects link cleanly

(
    cd ../core
    cargo build --release --lib
    cargo run -q --release --bin uniffi-bindgen -- generate \
        --library target/release/libtether_core.dylib --language swift --out-dir target/swift
)
mkdir -p .rust Sources/Tether/Generated
cp ../core/target/swift/tether_coreFFI.h Sources/TetherCoreFFI/
cp ../core/target/swift/tether_core.swift Sources/Tether/Generated/
# Only the static library goes in .rust/, so the app doesn't depend on the dylib at runtime.
cp ../core/target/release/libtether_core.a .rust/

swift build -c "$CONFIG"

APP=build/Tether.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp ".build/$CONFIG/Tether" "$APP/Contents/MacOS/"
cp Info.plist "$APP/Contents/"
# Releases set the version from their tag.
if [ -n "$TETHER_VERSION" ]; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $TETHER_VERSION" "$APP/Contents/Info.plist"
fi
ICONSET=build/AppIcon.iconset
mkdir -p "$ICONSET" "$APP/Contents/Resources"
for px in 16 32 128 256 512; do
  sips -z $px $px ../icon/icon.png --out "$ICONSET/icon_${px}x${px}.png" >/dev/null
  sips -z $((px * 2)) $((px * 2)) ../icon/icon.png --out "$ICONSET/icon_${px}x${px}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
codesign --force --sign - "$APP"
echo "Built $APP"
