#!/usr/bin/env bash
# tamux development script
# Usage: ./scripts/dev.sh [daemon|ui|cli|all]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cmd="${1:-all}"

case "$cmd" in
  daemon)
    echo "▶ Starting tamux-daemon in development mode..."
    cd "$PROJECT_ROOT"
    TAMUX_LOG=debug cargo run --bin tamux-daemon
    ;;
  ui)
    echo "▶ Starting Electron dev UI..."
    cd "$PROJECT_ROOT/frontend"
    npm install
    npm run dev:electron
    ;;
  frontend)
    echo "▶ Starting frontend dev server only..."
    cd "$PROJECT_ROOT/frontend"
    npm install
    npm run dev
    ;;
  cli)
    echo "▶ Building CLI..."
    cd "$PROJECT_ROOT"
    cargo build --bin tamux
    echo "✓ CLI built: target/debug/tamux"
    ;;
  all)
    echo "▶ Building all crates..."
    cd "$PROJECT_ROOT"
    cargo build
    echo ""
    echo "✓ All crates built."
    echo ""
    echo "To start the daemon:  ./scripts/dev.sh daemon"
    echo "To start the UI:      ./scripts/dev.sh ui"
    echo "To start frontend:    ./scripts/dev.sh frontend"
    ;;
  *)
    echo "Usage: $0 [daemon|ui|frontend|cli|all]"
    exit 1
    ;;
esac
