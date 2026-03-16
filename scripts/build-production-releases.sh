#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DIST_DIR="$PROJECT_ROOT/dist-release"

RUN_NATIVE=1
RUN_WINDOWS=1
SIGN=0

NATIVE_ARGS=()

usage() {
    cat <<'EOF'
Usage:
  ./scripts/build-production-releases.sh [options]

Rebuilds dist-release/ from scratch using the production release scripts.

Default behavior on Linux:
  - builds the native Linux release into dist-release/linux
  - builds the Windows cross-release into dist-release/windows when MinGW is available

Options:
  --sign             Enable signing in child scripts
  --native-only      Build only the native platform release
  --windows-only     Build only the Windows cross-release
  --skip-rust        Reuse existing Rust binaries
  --skip-frontend    Reuse existing frontend build output
  --skip-electron    Skip Electron packaging
  --target <triple>  Pass a Rust target triple to the native script
  --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign)
            SIGN=1
            shift
            ;;
        --native-only)
            RUN_NATIVE=1
            RUN_WINDOWS=0
            shift
            ;;
        --windows-only)
            RUN_NATIVE=0
            RUN_WINDOWS=1
            shift
            ;;
        --skip-rust|--skip-frontend|--skip-electron)
            NATIVE_ARGS+=("$1")
            shift
            ;;
        --target)
            [[ $# -ge 2 ]] || { echo "Missing value for --target" >&2; exit 1; }
            NATIVE_ARGS+=("$1" "$2")
            shift 2
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

OS="$(uname -s)"

if [[ $RUN_NATIVE -eq 1 && $RUN_WINDOWS -eq 1 && "$OS" == "Darwin" ]]; then
    echo "Windows cross-build is not supported by this wrapper on macOS." >&2
    echo "Use --native-only on macOS, or build Windows from Linux/WSL." >&2
    exit 1
fi

echo ""
echo "============================================================"
echo " tamux production release rebuild"
echo "============================================================"
echo ""
echo "This will recreate $DIST_DIR from scratch."

echo ""
echo "[preflight] Running setup checks..."
"$SCRIPT_DIR/setup.sh" --check --profile source --format text

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

run_native_release() {
    local cmd=("$SCRIPT_DIR/build-release.sh")
    if [[ $SIGN -eq 1 ]]; then
        cmd+=(--sign)
    fi
    cmd+=("${NATIVE_ARGS[@]}")

    echo ""
    echo "[native] Running ${cmd[*]}"
    "${cmd[@]}"
}

run_windows_release() {
    local cmd=("$SCRIPT_DIR/build-release-wsl.sh")
    if [[ $SIGN -eq 1 ]]; then
        cmd+=(--sign)
    fi

    if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo ""
        echo "[windows] Skipped: MinGW cross-compiler not found."
        echo "           Install with: sudo apt update && sudo apt install -y mingw-w64"
        return 0
    fi

    echo ""
    echo "[windows] Running ${cmd[*]}"
    "${cmd[@]}"
}

if [[ $RUN_NATIVE -eq 1 ]]; then
    run_native_release
fi

if [[ $RUN_WINDOWS -eq 1 ]]; then
    case "$OS" in
        Linux*)
            run_windows_release
            ;;
        *)
            echo ""
            echo "[windows] Skipped: Windows cross-release is only wired up for Linux/WSL in this wrapper."
            ;;
    esac
fi

echo ""
echo "============================================================"
echo " Production release rebuild complete"
echo "============================================================"

find "$DIST_DIR" -maxdepth 2 -type f | sort
