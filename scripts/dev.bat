@echo off
REM tamux development script for Windows
REM Usage: scripts\dev.bat [daemon|ui|cli|all]

set CMD=%1
if "%CMD%"=="" set CMD=all

if "%CMD%"=="daemon" (
    echo Starting tamux-daemon in development mode...
    set AMUX_LOG=debug
    cargo run --bin tamux-daemon
    goto :eof
)

if "%CMD%"=="ui" (
    echo Starting Electron dev UI...
    cd frontend
    call npm install
    call npm run dev:electron
    goto :eof
)

if "%CMD%"=="frontend" (
    echo Starting frontend dev server...
    cd frontend
    call npm install
    call npm run dev
    goto :eof
)

if "%CMD%"=="cli" (
    echo Building CLI...
    cargo build --bin tamux
    echo CLI built: target\debug\tamux.exe
    goto :eof
)

if "%CMD%"=="all" (
    echo Building all crates...
    cargo build
    echo.
    echo All crates built.
    echo.
    echo To start the daemon:  scripts\dev.bat daemon
    echo To start the UI:      scripts\dev.bat ui
    echo To start frontend:    scripts\dev.bat frontend
    goto :eof
)

echo Usage: scripts\dev.bat [daemon^|ui^|frontend^|cli^|all]
