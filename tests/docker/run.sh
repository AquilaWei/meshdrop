#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --bin meshdrop --locked
exec python3 tests/docker/scenario.py
