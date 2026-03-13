#!/usr/bin/env bash
# ============================================================================
# tamux release build (Linux / macOS / WSL)
#
# Usage:
#   ./scripts/build-release.sh                    Build without signing
#   ./scripts/build-release.sh --sign             Build and sign (GPG)
#   ./scripts/build-release.sh --skip-rust        Skip Rust compilation
#   ./scripts/build-release.sh --skip-frontend    Skip frontend build
#   ./scripts/build-release.sh --skip-electron    Skip Electron packaging
#   ./scripts/build-release.sh --target <triple>  Cross-compile for target
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUT_DIR="$PROJECT_ROOT/dist-release"

SIGN=0
SKIP_RUST=0
SKIP_FRONTEND=0
SKIP_ELECTRON=0
TARGET=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign)          SIGN=1; shift ;;
        --skip-rust)     SKIP_RUST=1; shift ;;
        --skip-frontend) SKIP_FRONTEND=1; shift ;;
        --skip-electron) SKIP_ELECTRON=1; shift ;;
        --target)        TARGET="$2"; shift 2 ;;
        *)               echo "Unknown option: $1"; exit 1 ;;
    esac
done

step=0
total=5

step_msg() {
    step=$((step + 1))
    echo ""
    echo "[$step/$total] $1"
}

ok_msg() {
    echo "  $1"
}

warn_msg() {
    echo "  WARNING: $1"
}

# Detect platform
OS="$(uname -s)"
EXE=""
if [[ "$OS" == *MINGW* ]] || [[ "$OS" == *MSYS* ]] || [[ "$OS" == *CYGWIN* ]]; then
    EXE=".exe"
fi

echo ""
echo "============================================================"
echo " tamux release build"
echo "============================================================"

# -----------------------------------------------------------
# Step 1: Rust
# -----------------------------------------------------------
if [[ $SKIP_RUST -eq 1 ]]; then
    step_msg "Skipping Rust build (--skip-rust)"
else
    step_msg "Building Rust binaries (release)..."
    cd "$PROJECT_ROOT"
    CARGO_ARGS=(build --release)
    if [[ -n "$TARGET" ]]; then
        CARGO_ARGS+=(--target "$TARGET")
    fi
    cargo "${CARGO_ARGS[@]}"
    ok_msg "Rust build complete"
fi

# -----------------------------------------------------------
# Step 2: Frontend
# -----------------------------------------------------------
if [[ $SKIP_FRONTEND -eq 1 ]]; then
    step_msg "Skipping frontend build (--skip-frontend)"
else
    step_msg "Building frontend..."
    cd "$PROJECT_ROOT/frontend"
    npm ci --silent 2>/dev/null || npm ci
    npm run build
    ok_msg "Frontend build complete"
fi

# -----------------------------------------------------------
# Step 3: Collect artifacts
# -----------------------------------------------------------
step_msg "Collecting artifacts..."
mkdir -p "$OUT_DIR"

if [[ -n "$TARGET" ]]; then
    TARGET_DIR="$PROJECT_ROOT/target/$TARGET/release"
else
    TARGET_DIR="$PROJECT_ROOT/target/release"
fi

for bin in tamux-daemon tamux tamux-mcp tamux-gateway; do
    if [[ -f "$TARGET_DIR/${bin}${EXE}" ]]; then
        cp "$TARGET_DIR/${bin}${EXE}" "$OUT_DIR/"
        ok_msg "Collected ${bin}${EXE}"
    fi
done

# Copy to frontend/dist for Electron bundling
if [[ -d "$PROJECT_ROOT/frontend/dist" ]]; then
    for bin in tamux-daemon tamux; do
        if [[ -f "$OUT_DIR/${bin}${EXE}" ]]; then
            cp "$OUT_DIR/${bin}${EXE}" "$PROJECT_ROOT/frontend/dist/"
        fi
    done
fi

# -----------------------------------------------------------
# Step 4: Code signing
# -----------------------------------------------------------
if [[ $SIGN -eq 1 ]]; then
    step_msg "Signing binaries..."

    sign_file() {
        local file="$1"
        [[ -f "$file" ]] || return 0

        local name
        name="$(basename "$file")"

        if [[ -n "$EXE" ]] && command -v signtool &>/dev/null; then
            # Windows signing via signtool
            if [[ -n "${AMUX_SIGN_CERT:-}" ]]; then
                signtool sign /f "$AMUX_SIGN_CERT" /p "${AMUX_SIGN_PASSWORD:-}" \
                    /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
                ok_msg "Signed $name (signtool/PFX)"
            elif [[ -n "${AMUX_SIGN_THUMBPRINT:-}" ]]; then
                signtool sign /sha1 "$AMUX_SIGN_THUMBPRINT" \
                    /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
                ok_msg "Signed $name (signtool/store)"
            else
                warn_msg "No Windows signing cert configured for $name"
            fi
        elif command -v gpg &>/dev/null; then
            # Linux/macOS signing via GPG
            gpg --detach-sign --armor "$file"
            ok_msg "Signed $name (GPG detached sig: ${name}.asc)"
        elif command -v codesign &>/dev/null && [[ "$OS" == "Darwin" ]]; then
            # macOS signing
            local identity="${AMUX_SIGN_IDENTITY:-}"
            if [[ -n "$identity" ]]; then
                codesign --sign "$identity" --timestamp --options runtime "$file"
                ok_msg "Signed $name (codesign)"
            else
                warn_msg "Set AMUX_SIGN_IDENTITY for macOS codesign"
            fi
        else
            warn_msg "No signing tool available for $name"
        fi
    }

    for f in "$OUT_DIR"/*; do
        [[ -f "$f" ]] && sign_file "$f"
    done
    ok_msg "Signing complete"
else
    step_msg "Skipping code signing (pass --sign to enable)"
fi

# -----------------------------------------------------------
# Step 5: Electron packaging
# -----------------------------------------------------------
if [[ $SKIP_ELECTRON -eq 1 ]]; then
    step_msg "Skipping Electron build (--skip-electron)"
else
    step_msg "Building Electron app..."
    cd "$PROJECT_ROOT/frontend"

    if [[ $SIGN -eq 1 ]]; then
        # Pass signing certs to electron-builder
        export CSC_LINK="${AMUX_SIGN_CERT:-}"
        export CSC_KEY_PASSWORD="${AMUX_SIGN_PASSWORD:-}"
    fi

    case "$OS" in
        Linux*)
            npx electron-builder --linux AppImage deb || warn_msg "Electron Linux build failed (non-fatal)"
            ;;
        Darwin*)
            npx electron-builder --mac dmg || warn_msg "Electron macOS build failed (non-fatal)"
            ;;
        *)
            npx electron-builder --win portable nsis || warn_msg "Electron Windows build failed (non-fatal)"
            ;;
    esac

    # Collect Electron artifacts
    RELEASE_DIR="$PROJECT_ROOT/frontend/release"
    if [[ -d "$RELEASE_DIR" ]]; then
        find "$RELEASE_DIR" -maxdepth 1 -type f \( -name "*.exe" -o -name "*.AppImage" -o -name "*.deb" -o -name "*.dmg" -o -name "*.rpm" \) 2>/dev/null | while read -r f; do
            cp "$f" "$OUT_DIR/"
            ok_msg "Electron: $(basename "$f")"
        done
    fi
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
    local_size=$(du -h "$f" | cut -f1)
    printf "  %-30s %s\n" "$(basename "$f")" "$local_size"
done
echo ""
if [[ $SIGN -eq 0 ]]; then
    echo "  Binaries are NOT signed. Run with --sign to sign."
fi
echo "============================================================"
