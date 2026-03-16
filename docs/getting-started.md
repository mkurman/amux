# tamux Getting Started

tamux is an AI-native terminal multiplexer with workspaces, panes, a Rust daemon, and an Electron desktop UI.

## Install Locations

- **Desktop app binaries (packaged):** inside application resources (`resources/bin` on desktop builds).
- **Runtime data directory:** `~/.tamux` (Unix) or `%LOCALAPPDATA%\tamux` (Windows).
- **Source builds:** binaries are produced in `target/debug` or `target/release`.

## Required Dependencies

### Source / Development workflow

- Rust toolchain (`cargo`)
- Node.js + npm
- git
- uv

### Packaged desktop runtime

- No extra hard-required dependencies.

## Optional (Recommended) Integrations

- aline (used for OneContext history recall and richer agent memory/bootstrap behavior)

## Setup Preflight

Run preflight checks before building or running:

```bash
./scripts/setup.sh --check --profile source
```

Windows PowerShell:

```powershell
.\scripts\setup.ps1 -Check -Profile source
```

Desktop-only runtime preflight:

```bash
./scripts/setup.sh --check --profile desktop
```

## Install Hints

- Install `uv`: `curl -LsSf https://astral.sh/uv/install.sh | sh`
- Install `aline`: `uv tool install aline-ai`

On Windows:

- Install Node.js LTS: `winget install OpenJS.NodeJS.LTS`
- Install Rust: `winget install Rustlang.Rustup`
- Install git: `winget install Git.Git`
