#!/bin/sh
# Builds the Rust core, generates its Swift bindings, builds the app and bundles build/MyDevice.app.
set -e
cd "$(dirname "$0")"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
CONFIG="${CONFIG:-release}"
export MACOSX_DEPLOYMENT_TARGET=26.0  # match Package.swift, so the Rust objects link cleanly

(
    cd ../core
    cargo build --release --lib
    cargo run -q --release --bin uniffi-bindgen -- generate \
        --library target/release/libmydevice_core.dylib --language swift --out-dir target/swift
)
mkdir -p .rust Sources/MyDevice/Generated
cp ../core/target/swift/mydevice_coreFFI.h Sources/MyDeviceCoreFFI/
cp ../core/target/swift/mydevice_core.swift Sources/MyDevice/Generated/
# Only the static library goes in .rust/, so the app doesn't depend on the dylib at runtime.
cp ../core/target/release/libmydevice_core.a .rust/

swift build -c "$CONFIG"

APP=build/MyDevice.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp ".build/$CONFIG/MyDevice" "$APP/Contents/MacOS/"
cp Info.plist "$APP/Contents/"
codesign --force --sign - "$APP"
echo "Built $APP"
