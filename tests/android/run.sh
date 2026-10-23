#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
export NDK_HOME=${NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}
export JAVA_HOME=${JAVA_HOME:-$HOME/.local/share/meshdrop-android/jdk-17}
if [[ ! -x "$JAVA_HOME/bin/java" ]]; then
  echo 'Missing JDK 17. Download Temurin 17 from https://adoptium.net/temurin/releases/?version=17 into ~/.local/share/meshdrop-android/jdk-17 or set JAVA_HOME.' >&2
  exit 1
fi
export MESHDROP_EMULATOR=${MESHDROP_EMULATOR:-$ANDROID_HOME/emulator/emulator}
if [[ ! -x "$MESHDROP_EMULATOR" || ! -e /dev/kvm ]]; then
  echo 'Android SDK emulator and accessible /dev/kvm are required; see tests/android/README.md.' >&2
  exit 1
fi
# Reserve the fixed NAT/CDP ports for this runner, never a user's existing AVD.
exec 9>"/tmp/meshdrop-android-${UID}.lock"
flock -n 9 || { echo 'Another meshdrop Android runner is active.' >&2; exit 1; }
python3 -m unittest discover -s tests/android -p 'test_*.py' && cargo build -p meshdrop-cli --locked && (cd crates/meshdrop-app && cargo tauri android build --debug --target x86_64) && crates/meshdrop-app/gen/android/gradlew -p crates/meshdrop-app/gen/android :app:testUniversalDebugUnitTest && crates/meshdrop-app/gen/android/gradlew -p tests/android/share-fixture assembleDebug && python3 tests/android/check_apk.py && python3 tests/android/scenario.py
