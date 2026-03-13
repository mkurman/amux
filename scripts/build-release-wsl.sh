#!/usr/bin/env bash
# ============================================================================
# tamux release build for Windows via WSL cross-compilation
#
# This script cross-compiles from WSL to Windows (x86_64-pc-windows-gnu),
# then builds the Electron app with optional code signing.
#
# Usage:
#   ./scripts/build-release-wsl.sh              Build without signing
#   ./scripts/build-release-wsl.sh --sign       Build and sign all binaries
#
# Signing options (env vars):
#   AMUX_SIGN_CERT       Path to PFX certificate file
#   AMUX_SIGN_PASSWORD   PFX certificate password
#   AMUX_SIGN_THUMBPRINT Certificate thumbprint (alternative to PFX)
#
# Prerequisites:
#   - WSL2 with Rust toolchain
#   - MinGW cross-compiler: sudo apt install mingw-w64
#   - Node.js + npm in WSL
#   - (Optional) signtool.exe accessible via /mnt/c/... or WSL interop
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUT_DIR="$PROJECT_ROOT/dist-release"
TARGET="x86_64-pc-windows-gnu"

SIGN=0
[[ "${1:-}" == "--sign" ]] && SIGN=1

echo ""
echo "============================================================"
echo " tamux Windows release build (WSL cross-compilation)"
echo "============================================================"

# -----------------------------------------------------------
# Preflight checks
# -----------------------------------------------------------
echo ""
echo "[0/5] Checking prerequisites..."

if ! command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    echo "  ERROR: MinGW not found. Install with:"
    echo "    sudo apt update && sudo apt install -y mingw-w64"
    exit 1
fi
echo "  MinGW: OK"

if ! rustup target list --installed | grep -q "$TARGET"; then
    echo "  Adding Rust target $TARGET..."
    rustup target add "$TARGET"
fi
echo "  Rust target: $TARGET OK"

# -----------------------------------------------------------
# Step 1: Cross-compile Rust
# -----------------------------------------------------------
echo ""
echo "[1/5] Cross-compiling Rust binaries ($TARGET)..."
cd "$PROJECT_ROOT"
cargo build --release --target "$TARGET"
echo "  Done."

# -----------------------------------------------------------
# Step 2: Build frontend
# -----------------------------------------------------------
echo ""
echo "[2/5] Building frontend..."
cd "$PROJECT_ROOT/frontend"
npm ci --silent 2>/dev/null || npm ci
npm run build
echo "  Done."

# -----------------------------------------------------------
# Step 3: Collect artifacts
# -----------------------------------------------------------
echo ""
echo "[3/5] Collecting artifacts..."
mkdir -p "$OUT_DIR"

TARGET_DIR="$PROJECT_ROOT/target/$TARGET/release"

for bin in tamux-daemon tamux tamux-mcp tamux-gateway; do
    if [[ -f "$TARGET_DIR/${bin}.exe" ]]; then
        cp "$TARGET_DIR/${bin}.exe" "$OUT_DIR/"
        echo "  Collected ${bin}.exe"
    fi
done

# Copy daemon + CLI to frontend/dist for Electron
cp "$OUT_DIR/tamux-daemon.exe" "$PROJECT_ROOT/frontend/dist/" 2>/dev/null || true
cp "$OUT_DIR/tamux.exe" "$PROJECT_ROOT/frontend/dist/" 2>/dev/null || true

# -----------------------------------------------------------
# Step 4: Code signing (optional)
# -----------------------------------------------------------
echo ""
if [[ $SIGN -eq 1 ]]; then
    echo "[4/5] Signing binaries..."

    # Try to find signtool.exe via WSL interop
    SIGNTOOL=""
    if [[ -d "/mnt/c/Program Files (x86)" ]]; then
        SIGNTOOL=$(find "/mnt/c/Program Files (x86)/" -name "signtool.exe" -type f 2>/dev/null | head -1)
    fi

    if [[ -z "$SIGNTOOL" ]]; then
        # Fall back to PATH
        if command -v signtool.exe &>/dev/null; then
            SIGNTOOL="signtool.exe"
        fi
    fi

    echo "  Using signtool: ${SIGNTOOL:-NOT FOUND}"

    sign_file() {
        local file="$1"
        local name
        name="$(basename "$file")"

        if [[ -z "$SIGNTOOL" ]]; then
            echo "  WARNING: signtool.exe not found. Skipping $name."
            return
        fi

        if [[ -n "${AMUX_SIGN_CERT:-}" ]]; then
            "$SIGNTOOL" sign /f "$AMUX_SIGN_CERT" /p "${AMUX_SIGN_PASSWORD:-}" \
                /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
            echo "  Signed $name (PFX)"
        elif [[ -n "${AMUX_SIGN_THUMBPRINT:-}" ]]; then
            "$SIGNTOOL" sign /sha1 "$AMUX_SIGN_THUMBPRINT" \
                /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
            echo "  Signed $name (cert store)"
        else
            echo "  WARNING: No signing cert configured for $name."
            echo "           Set AMUX_SIGN_CERT + AMUX_SIGN_PASSWORD"
            echo "           or AMUX_SIGN_THUMBPRINT."
        fi
    }

    for f in "$OUT_DIR"/*.exe; do
        [[ -f "$f" ]] && sign_file "$f"
    done
    echo "  Done."
else
    echo "[4/5] Skipping code signing (pass --sign to enable)."
fi

# -----------------------------------------------------------
# Step 5: Build Electron app
# -----------------------------------------------------------
echo ""
echo "[5/5] Building Electron app..."
cd "$PROJECT_ROOT/frontend"

if [[ $SIGN -eq 1 && -n "${AMUX_SIGN_CERT:-}" ]]; then
    export CSC_LINK="$AMUX_SIGN_CERT"
    export CSC_KEY_PASSWORD="${AMUX_SIGN_PASSWORD:-}"
fi

npx electron-builder --win portable nsis

# Collect Electron artifacts
RELEASE_DIR="$PROJECT_ROOT/frontend/release"
if [[ -d "$RELEASE_DIR" ]]; then
    for f in "$RELEASE_DIR"/*.exe; do
        [[ -f "$f" ]] || continue
        cp "$f" "$OUT_DIR/"
        echo "  Collected $(basename "$f")"
    done
fi

# -----------------------------------------------------------
# Summary
# -----------------------------------------------------------
echo ""
echo "============================================================"
echo " Build complete!"
echo "============================================================"
echo ""
echo "  Output: $OUT_DIR/"
echo ""
for f in "$OUT_DIR"/*; do
    [[ -f "$f" ]] || continue
    size=$(du -h "$f" | cut -f1)
    printf "  %-30s %s\n" "$(basename "$f")" "$size"
done
echo ""
if [[ $SIGN -eq 0 ]]; then
    echo "  Binaries are NOT signed. Run with --sign to sign."
fi
echo "============================================================"
