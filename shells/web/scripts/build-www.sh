#!/usr/bin/env bash
# Builds the self-contained static artifact into shells/web/www/pkg/
# (no CDN, no runtime fetches). Generated output; the repo carries script + sources.
# Usage (from anywhere): bash shells/web/scripts/build-www.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."

# wasm-bindgen CLI and crate must stay in lockstep (pinned 0.2.121).
REQUIRED_WASM_BINDGEN="0.2.121"

crate_version="$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock \
  | sed -n 's/^version = "\(.*\)"$/\1/p' || true)"
if [ "$crate_version" != "$REQUIRED_WASM_BINDGEN" ]; then
  echo "error: wasm-bindgen crate in Cargo.lock is '${crate_version:-missing}', expected $REQUIRED_WASM_BINDGEN" >&2
  exit 1
fi

cli_version="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$cli_version" != "$REQUIRED_WASM_BINDGEN" ]; then
  echo "error: wasm-bindgen CLI is '$cli_version', expected $REQUIRED_WASM_BINDGEN" >&2
  echo "       install it with: cargo install wasm-bindgen-cli --version $REQUIRED_WASM_BINDGEN" >&2
  exit 1
fi

echo "[1/3] cargo build (wasm32, release)"
cargo build -p room-shell-web --target wasm32-unknown-unknown --release

echo "[2/3] wasm-bindgen --target web"
wasm-bindgen --target web --out-dir shells/web/www/pkg --no-typescript \
  target/wasm32-unknown-unknown/release/room_shell_web.wasm

echo "[3/3] artifact size"
ls -l shells/web/www/pkg/room_shell_web_bg.wasm
echo "Local preview: python -m http.server 8000 --directory shells/web/www"
