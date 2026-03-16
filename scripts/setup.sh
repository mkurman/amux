#!/usr/bin/env bash
set -euo pipefail

PROFILE="source"
OUTPUT_FORMAT="text"
CHECK_MODE=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_MODE=1
            shift
            ;;
        --profile)
            PROFILE="${2:-}"
            shift 2
            ;;
        --format)
            OUTPUT_FORMAT="${2:-}"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 2
            ;;
    esac
done

if [[ "$PROFILE" != "source" && "$PROFILE" != "desktop" ]]; then
    echo "Invalid --profile '$PROFILE' (expected source|desktop)" >&2
    exit 2
fi

if [[ "$OUTPUT_FORMAT" != "text" && "$OUTPUT_FORMAT" != "json" ]]; then
    echo "Invalid --format '$OUTPUT_FORMAT' (expected text|json)" >&2
    exit 2
fi

PLATFORM="linux"
case "$(uname -s)" in
    Darwin*)
        PLATFORM="macos"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        PLATFORM="windows"
        ;;
esac

declare -a REQUIRED_DEPS
declare -a OPTIONAL_DEPS=(
    "aline"
    "tamux-mcp"
    "hermes"
    "openclaw"
)

if [[ "$PROFILE" == "source" ]]; then
    REQUIRED_DEPS=("cargo" "node" "npm" "git" "uv")
else
    REQUIRED_DEPS=()
fi

has_command() {
    command -v "$1" >/dev/null 2>&1
}

resolve_path() {
    command -v "$1" 2>/dev/null || true
}

json_escape() {
    local text="$1"
    text="${text//\\/\\\\}"
    text="${text//\"/\\\"}"
    text="${text//$'\n'/\\n}"
    text="${text//$'\r'/\\r}"
    text="${text//$'\t'/\\t}"
    printf '%s' "$text"
}

install_hint() {
    local dep="$1"
    case "$dep" in
        cargo)
            case "$PLATFORM" in
                linux)
                    echo "curl https://sh.rustup.rs -sSf | sh"
                    ;;
                macos)
                    echo "curl https://sh.rustup.rs -sSf | sh"
                    ;;
                windows)
                    echo "winget install Rustlang.Rustup"
                    ;;
            esac
            ;;
        node|npm)
            case "$PLATFORM" in
                linux)
                    echo "sudo apt update && sudo apt install -y nodejs npm"
                    ;;
                macos)
                    echo "brew install node"
                    ;;
                windows)
                    echo "winget install OpenJS.NodeJS.LTS"
                    ;;
            esac
            ;;
        git)
            case "$PLATFORM" in
                linux)
                    echo "sudo apt update && sudo apt install -y git"
                    ;;
                macos)
                    echo "brew install git"
                    ;;
                windows)
                    echo "winget install Git.Git"
                    ;;
            esac
            ;;
        uv)
            case "$PLATFORM" in
                linux|macos)
                    echo "curl -LsSf https://astral.sh/uv/install.sh | sh"
                    ;;
                windows)
                    echo "powershell -ExecutionPolicy ByPass -c \"irm https://astral.sh/uv/install.ps1 | iex\""
                    ;;
            esac
            ;;
        aline)
            echo "uv tool install aline-ai"
            ;;
        tamux-mcp)
            echo "cargo build --release -p tamux-mcp"
            ;;
        hermes)
            echo "python3 -m pip install \"hermes-agent[all]\""
            ;;
        openclaw)
            echo "npm install -g openclaw"
            ;;
        *)
            echo "No install hint available"
            ;;
    esac
}

declare -a REQUIRED_ROWS=()
declare -a OPTIONAL_ROWS=()
declare -a MISSING_REQUIRED=()

for dep in "${REQUIRED_DEPS[@]}"; do
    if has_command "$dep"; then
        dep_path="$(resolve_path "$dep")"
        REQUIRED_ROWS+=("$dep|1|$dep_path|$(install_hint "$dep")")
    else
        REQUIRED_ROWS+=("$dep|0||$(install_hint "$dep")")
        MISSING_REQUIRED+=("$dep")
    fi
done

for dep in "${OPTIONAL_DEPS[@]}"; do
    if has_command "$dep"; then
        dep_path="$(resolve_path "$dep")"
        OPTIONAL_ROWS+=("$dep|1|$dep_path|$(install_hint "$dep")")
    else
        OPTIONAL_ROWS+=("$dep|0||$(install_hint "$dep")")
    fi
done

if [[ "$OUTPUT_FORMAT" == "text" ]]; then
    echo "tamux setup preflight"
    echo "Profile:  $PROFILE"
    echo "Platform: $PLATFORM"
    echo ""
    echo "Required dependencies:"
    for row in "${REQUIRED_ROWS[@]}"; do
        IFS='|' read -r dep found dep_path hint <<<"$row"
        if [[ "$found" == "1" ]]; then
            echo "  [ok]    $dep ($dep_path)"
        else
            echo "  [missing] $dep"
            echo "           install: $hint"
        fi
    done
    echo ""
    echo "Optional dependencies:"
    for row in "${OPTIONAL_ROWS[@]}"; do
        IFS='|' read -r dep found dep_path hint <<<"$row"
        if [[ "$found" == "1" ]]; then
            echo "  [ok]    $dep ($dep_path)"
        else
            echo "  [optional-missing] $dep"
            echo "                     install: $hint"
        fi
    done

    if [[ "${#MISSING_REQUIRED[@]}" -gt 0 ]]; then
        echo ""
        echo "Missing required dependencies: ${MISSING_REQUIRED[*]}"
    else
        echo ""
        echo "All required dependencies are installed."
    fi
else
    printf '{'
    printf '"platform":"%s",' "$(json_escape "$PLATFORM")"
    printf '"profile":"%s",' "$(json_escape "$PROFILE")"
    printf '"required":['
    for i in "${!REQUIRED_ROWS[@]}"; do
        IFS='|' read -r dep found dep_path hint <<<"${REQUIRED_ROWS[$i]}"
        [[ "$i" -gt 0 ]] && printf ','
        printf '{'
        printf '"name":"%s",' "$(json_escape "$dep")"
        printf '"found":%s,' "$([[ "$found" == "1" ]] && echo "true" || echo "false")"
        printf '"path":"%s",' "$(json_escape "$dep_path")"
        printf '"install_hint":"%s"' "$(json_escape "$hint")"
        printf '}'
    done
    printf '],'
    printf '"optional":['
    for i in "${!OPTIONAL_ROWS[@]}"; do
        IFS='|' read -r dep found dep_path hint <<<"${OPTIONAL_ROWS[$i]}"
        [[ "$i" -gt 0 ]] && printf ','
        printf '{'
        printf '"name":"%s",' "$(json_escape "$dep")"
        printf '"found":%s,' "$([[ "$found" == "1" ]] && echo "true" || echo "false")"
        printf '"path":"%s",' "$(json_escape "$dep_path")"
        printf '"install_hint":"%s"' "$(json_escape "$hint")"
        printf '}'
    done
    printf '],'
    printf '"missing_required":['
    for i in "${!MISSING_REQUIRED[@]}"; do
        [[ "$i" -gt 0 ]] && printf ','
        printf '"%s"' "$(json_escape "${MISSING_REQUIRED[$i]}")"
    done
    printf ']'
    printf '}\n'
fi

if [[ "$CHECK_MODE" == "1" && "${#MISSING_REQUIRED[@]}" -gt 0 ]]; then
    exit 1
fi
