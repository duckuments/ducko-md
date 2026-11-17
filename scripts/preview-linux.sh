#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $(uname -s) == Linux ]] || { echo 'Run this preview inside Linux.' >&2; exit 1; }
cargo build --release --locked
export DUCKO_HOME="${DUCKO_HOME:-$(mktemp -d "${TMPDIR:-/tmp}/ducko-linux-preview.XXXXXX")}"
printf 'Preview library: %s\n' "$DUCKO_HOME"
exec "${CARGO_TARGET_DIR:-target}/release/ducko"
