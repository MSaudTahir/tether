#!/bin/sh
# Rebuilds helper/helper.dex from Helper.java (the Rust core embeds it). Needs a JDK and the Android SDK (platform + build-tools).
set -e
cd "$(dirname "$0")"
SDK=${ANDROID_HOME:-$HOME/Library/Android/sdk}
JAR=$(ls -d "$SDK"/platforms/android-*/android.jar | sort -V | tail -1)
D8=$(ls -d "$SDK"/build-tools/*/d8 | sort -V | tail -1)
OUT=$(mktemp -d)

javac --release 11 -classpath "$JAR" -d "$OUT" Helper.java
"$D8" --release --min-api 21 --lib "$JAR" --output "$OUT" "$OUT"/app/mydevice/*.class
cp "$OUT/classes.dex" helper.dex
cp helper.dex ../python/mydevice/helper.dex  # the Python app keeps its own copy until it is retired
rm -rf "$OUT"
echo "Built helper/helper.dex"
