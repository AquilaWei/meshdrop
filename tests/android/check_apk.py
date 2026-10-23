"""Assert the built APK inherits Cargo's version and retains the approved SDK range."""
import os
from pathlib import Path
import re
import subprocess
import tomllib

manifest = tomllib.loads(Path('Cargo.toml').read_text())
expected = manifest['workspace']['package']['version']
sdk = Path(os.environ.get('ANDROID_HOME', str(Path.home() / 'Android/Sdk')))
apk = Path('crates/meshdrop-app/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk')
badging = subprocess.check_output([str(sdk/'build-tools/36.0.0/aapt'), 'dump', 'badging', str(apk)], text=True)
actual = re.search(r"versionName='([^']+)'", badging).group(1)
assert actual == expected, f'APK version {actual} differs from workspace {expected}'
assert "sdkVersion:'24'" in badging, 'Approved minSdk changed'
assert "targetSdkVersion:'36'" in badging, 'Approved targetSdk changed'
print(f'APK version {actual} matches the workspace; minSdk 24 / targetSdk 36 retained')
