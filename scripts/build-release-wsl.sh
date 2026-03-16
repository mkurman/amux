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
#   TAMUX_SIGN_CERT       Path to PFX certificate file
#   TAMUX_SIGN_PASSWORD   PFX certificate password
#   TAMUX_SIGN_THUMBPRINT Certificate thumbprint (alternative to PFX)
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
OUT_DIR="$PROJECT_ROOT/dist-release/windows"
TARGET="x86_64-pc-windows-gnu"
APP_VERSION="$(sed -nE 's/^[[:space:]]*"version":[[:space:]]*"([^"]+)".*/\1/p' "$PROJECT_ROOT/frontend/package.json" | head -1)"
if [[ -z "$APP_VERSION" ]]; then
    APP_VERSION="0.0.0"
fi
ARCH="x64"

SIGN=0
[[ "${1:-}" == "--sign" ]] && SIGN=1

checksum_file() {
    local file="$1"
    if command -v sha256sum &>/dev/null; then
        sha256sum "$file" | awk '{print $1}'
    elif command -v shasum &>/dev/null; then
        shasum -a 256 "$file" | awk '{print $1}'
    else
        python3 - "$file" <<'PY'
import hashlib
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
print(hashlib.sha256(path.read_bytes()).hexdigest())
PY
    fi
}

generate_release_notes_if_missing() {
    local notes_file="$1"
    shift
    local artifacts=("$@")

    [[ -f "$notes_file" ]] && return 0

    {
        echo "# tamux ${APP_VERSION} Windows Release Notes"
        echo ""
        echo "Built on $(date -u +"%Y-%m-%d %H:%M UTC") via WSL cross-compilation."
        echo ""
        echo "## Highlights"
        echo ""
        echo "- Rebrand alignment across runtime, packaging, and release artifacts."
        echo "- Updated release packaging flow with zipped bundles and SHA-256 manifests."
        echo "- Browser panel layout and resize fixes in the Electron frontend."
        echo ""
        echo "## Included Artifacts"
        echo ""
        for artifact in "${artifacts[@]}"; do
            echo "- ${artifact}"
        done
    } > "$notes_file"
}

write_checksums_file() {
    local output_file="$1"
    shift
    local artifacts=("$@")

    : > "$output_file"
    for artifact in "${artifacts[@]}"; do
        local hash
        hash="$(checksum_file "$OUT_DIR/$artifact")"
        printf "%s  %s\n" "$hash" "$artifact" >> "$output_file"
    done
}

create_bundle_zip() {
    local zip_path="$1"
    shift
    local files=("$@")

    python3 - "$zip_path" "$OUT_DIR" "${files[@]}" <<'PY'
import pathlib
import sys
import zipfile

zip_path = pathlib.Path(sys.argv[1])
base_dir = pathlib.Path(sys.argv[2])
files = sys.argv[3:]

with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for relative in files:
        archive.write(base_dir / relative, arcname=relative)
PY
}

echo ""
echo "============================================================"
echo " tamux Windows release build (WSL cross-compilation)"
echo "============================================================"

# -----------------------------------------------------------
# Preflight checks
# -----------------------------------------------------------
echo ""
echo "[0/6] Checking prerequisites..."

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

echo ""
echo "  Running tamux setup preflight..."
"$SCRIPT_DIR/setup.sh" --check --profile source --format text

# -----------------------------------------------------------
# Step 1: Cross-compile Rust
# -----------------------------------------------------------
echo ""
echo "[1/6] Cross-compiling Rust binaries ($TARGET)..."
cd "$PROJECT_ROOT"
cargo build --release --target "$TARGET"
echo "  Done."

# -----------------------------------------------------------
# Step 2: Build frontend
# -----------------------------------------------------------
echo ""
echo "[2/6] Building frontend..."
cd "$PROJECT_ROOT/frontend"
npm ci --silent 2>/dev/null || npm ci
npm run build
echo "  Done."

# -----------------------------------------------------------
# Step 3: Collect artifacts
# -----------------------------------------------------------
echo ""
echo "[3/6] Collecting artifacts..."
mkdir -p "$OUT_DIR"
find "$OUT_DIR" -maxdepth 1 -type f \( -name "tamux*" -o -name "amux*" -o -name "SHA256SUMS*.txt" -o -name "RELEASE_NOTES*.md" \) -delete 2>/dev/null || true

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
if [[ -f "$PROJECT_ROOT/docs/getting-started.md" ]]; then
    cp "$PROJECT_ROOT/docs/getting-started.md" "$OUT_DIR/GETTING_STARTED.md"
    cp "$PROJECT_ROOT/docs/getting-started.md" "$PROJECT_ROOT/frontend/dist/GETTING_STARTED.md"
    echo "  Collected GETTING_STARTED.md"
fi

# -----------------------------------------------------------
# Step 4: Code signing (optional)
# -----------------------------------------------------------
echo ""
if [[ $SIGN -eq 1 ]]; then
    echo "[4/6] Signing binaries..."

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

        if [[ -n "${TAMUX_SIGN_CERT:-${AMUX_SIGN_CERT:-}}" ]]; then
            "$SIGNTOOL" sign /f "${TAMUX_SIGN_CERT:-${AMUX_SIGN_CERT:-}}" /p "${TAMUX_SIGN_PASSWORD:-${AMUX_SIGN_PASSWORD:-}}" \
                /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
            echo "  Signed $name (PFX)"
        elif [[ -n "${TAMUX_SIGN_THUMBPRINT:-${AMUX_SIGN_THUMBPRINT:-}}" ]]; then
            "$SIGNTOOL" sign /sha1 "${TAMUX_SIGN_THUMBPRINT:-${AMUX_SIGN_THUMBPRINT:-}}" \
                /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$file"
            echo "  Signed $name (cert store)"
        else
            echo "  WARNING: No signing cert configured for $name."
            echo "           Set TAMUX_SIGN_CERT + TAMUX_SIGN_PASSWORD"
            echo "           or TAMUX_SIGN_THUMBPRINT."
        fi
    }

    for f in "$OUT_DIR"/*.exe; do
        [[ -f "$f" ]] && sign_file "$f"
    done
    echo "  Done."
else
    echo "[4/6] Skipping code signing (pass --sign to enable)."
fi

# -----------------------------------------------------------
# Step 5: Build Electron app
# -----------------------------------------------------------
echo ""
echo "[5/6] Building Electron app..."
cd "$PROJECT_ROOT/frontend"
find "$PROJECT_ROOT/frontend/release" -maxdepth 1 -type f \( -name "tamux*" -o -name "amux*" \) -delete 2>/dev/null || true

if [[ $SIGN -eq 1 && -n "${TAMUX_SIGN_CERT:-${AMUX_SIGN_CERT:-}}" ]]; then
    export CSC_LINK="${TAMUX_SIGN_CERT:-${AMUX_SIGN_CERT:-}}"
    export CSC_KEY_PASSWORD="${TAMUX_SIGN_PASSWORD:-${AMUX_SIGN_PASSWORD:-}}"
fi

npx electron-builder --win portable nsis

# Collect Electron artifacts
RELEASE_DIR="$PROJECT_ROOT/frontend/release"
if [[ -d "$RELEASE_DIR" ]]; then
    for f in "$RELEASE_DIR"/tamux*.exe; do
        [[ -f "$f" ]] || continue
        cp "$f" "$OUT_DIR/"
        echo "  Collected $(basename "$f")"
    done
fi

# -----------------------------------------------------------
# Step 6: Package bundle + checksums + notes
# -----------------------------------------------------------
echo ""
echo "[6/6] Packaging release bundle..."

bundle_artifacts=()
for file in "$OUT_DIR"/*; do
    [[ -f "$file" ]] || continue
    base_name="$(basename "$file")"
    case "$base_name" in
        *.zip|SHA256SUMS*.txt|RELEASE_NOTES*.md) continue ;;
    esac
    bundle_artifacts+=("$base_name")
done

if [[ ${#bundle_artifacts[@]} -gt 0 ]]; then
    notes_file="$OUT_DIR/RELEASE_NOTES.md"
    checksums_file="$OUT_DIR/SHA256SUMS.txt"
    bundle_file="$OUT_DIR/tamux-${APP_VERSION}-windows-${ARCH}.zip"

    generate_release_notes_if_missing "$notes_file" "${bundle_artifacts[@]}"
    write_checksums_file "$checksums_file" "${bundle_artifacts[@]}"
    create_bundle_zip "$bundle_file" "${bundle_artifacts[@]}" "$(basename "$checksums_file")" "$(basename "$notes_file")"

    echo "  Created $(basename "$checksums_file")"
    echo "  Created $(basename "$notes_file")"
    echo "  Created $(basename "$bundle_file")"
else
    echo "  WARNING: No release artifacts available to package"
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
