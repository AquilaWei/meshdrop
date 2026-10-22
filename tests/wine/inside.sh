#!/usr/bin/env bash
set -euo pipefail
cargo test --workspace --exclude meshdrop-app --locked --target x86_64-pc-windows-gnu
# Compile-only: the desktop app needs WebView2, which Wine cannot run (see README).
cargo check -p meshdrop-app --locked --target x86_64-pc-windows-gnu
