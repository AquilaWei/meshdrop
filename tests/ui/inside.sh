#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build -p meshdrop-app -p meshdrop-cli --locked
exec dbus-run-session -- xvfb-run -a -s '-screen 0 1280x1024x24' bash -c 'dunst > /tmp/dunst.log 2>&1 & python3 tests/ui/regression.py && python3 tests/ui/scenario.py && GTK_THEME=Adwaita:light MESHDROP_SCREEN_THEME=light python3 tests/ui/screenshots.py && GTK_THEME=Adwaita:dark MESHDROP_SCREEN_THEME=dark python3 tests/ui/screenshots.py'
