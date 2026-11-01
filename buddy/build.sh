#!/bin/sh
# Builds buddy.apk (Tether Buddy) with the Android SDK's command-line tools; no Gradle.
# Needs ANDROID_HOME (or ~/Library/Android/sdk) with a platform and build-tools.
set -e
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
TOOLS="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
JAR="$(ls -d "$SDK"/platforms/android-* | sort -V | tail -1)/android.jar"
OUT=build
rm -rf "$OUT" && mkdir -p "$OUT/classes"

"$TOOLS/aapt2" link -o "$OUT/unsigned.apk" -I "$JAR" --manifest AndroidManifest.xml
javac --release 11 -cp "$JAR" -d "$OUT/classes" $(find src -name '*.java')
"$TOOLS/d8" --min-api 21 --lib "$JAR" --output "$OUT" $(find "$OUT/classes" -name '*.class')
(cd "$OUT" && zip -q unsigned.apk classes.dex)
"$TOOLS/zipalign" -f 4 "$OUT/unsigned.apk" "$OUT/aligned.apk"
# Phones only update Buddy if it's signed with the same key as the installed one. The key isn't in
# git (keep a backup); without one, make a local key, which can't update a Buddy signed by another.
KEY="${BUDDY_KEYSTORE:-tether-buddy.keystore}"
PASS="${BUDDY_KEYSTORE_PASSWORD:-tether-buddy}"
if [ ! -f "$KEY" ]; then
  echo "No signing key at $KEY; making a new one."
  keytool -genkeypair -keystore "$KEY" -storepass "$PASS" -keypass "$PASS" -alias buddy -keyalg RSA \
    -keysize 2048 -validity 36500 -dname "CN=Tether Buddy, O=Tether" >/dev/null 2>&1
fi
"$TOOLS/apksigner" sign --ks "$KEY" --ks-pass "pass:$PASS" --out buddy.apk "$OUT/aligned.apk"
rm -rf "$OUT" buddy.apk.idsig
echo "Built buddy/buddy.apk"
