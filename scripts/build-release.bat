@echo off
REM ============================================================================
REM tamux release build for Windows (native)
REM
REM Prerequisites:
REM   - Rust toolchain (rustup)
REM   - Node.js + npm
REM   - (Optional) signtool.exe in PATH for code signing
REM   - (Optional) Set TAMUX_SIGN_CERT and TAMUX_SIGN_PASSWORD for signing
REM
REM Usage:
REM   scripts\build-release.bat              Build without signing
REM   scripts\build-release.bat --sign       Build and sign all binaries
REM ============================================================================
setlocal enabledelayedexpansion

set SIGN=0
if "%~1"=="--sign" set SIGN=1

set PROJECT_ROOT=%~dp0..
set OUT_DIR=%PROJECT_ROOT%\dist-release

echo.
echo ============================================================
echo  tamux release build
echo ============================================================
echo.

REM -----------------------------------------------------------
REM Setup preflight
REM -----------------------------------------------------------
echo [0/5] Running setup preflight...
powershell -NoProfile -ExecutionPolicy Bypass -File "%PROJECT_ROOT%\scripts\setup.ps1" -Check -Profile source -Format text
if errorlevel 1 (
    echo ERROR: setup preflight failed.
    exit /b 1
)
echo       Setup preflight complete.

REM -----------------------------------------------------------
REM Step 1: Build Rust workspace (daemon + CLI + MCP + gateway)
REM -----------------------------------------------------------
echo [1/5] Building Rust binaries (release)...
cd /d "%PROJECT_ROOT%"
cargo build --release
if errorlevel 1 (
    echo ERROR: Cargo build failed.
    exit /b 1
)
echo       Done.

REM -----------------------------------------------------------
REM Step 2: Build frontend
REM -----------------------------------------------------------
echo [2/5] Building frontend...
cd /d "%PROJECT_ROOT%\frontend"
call npm ci --silent
call npm run build
if errorlevel 1 (
    echo ERROR: Frontend build failed.
    exit /b 1
)
echo       Done.

REM -----------------------------------------------------------
REM Step 3: Collect binaries
REM -----------------------------------------------------------
echo [3/5] Collecting artifacts...
if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"
del /Q "%OUT_DIR%\tamux*" >nul 2>nul
del /Q "%OUT_DIR%\amux*" >nul 2>nul

copy /Y "%PROJECT_ROOT%\target\release\tamux-daemon.exe" "%OUT_DIR%\" >nul
copy /Y "%PROJECT_ROOT%\target\release\tamux.exe"        "%OUT_DIR%\" >nul
copy /Y "%PROJECT_ROOT%\target\release\tamux-mcp.exe"    "%OUT_DIR%\" >nul 2>nul
copy /Y "%PROJECT_ROOT%\target\release\tamux-gateway.exe" "%OUT_DIR%\" >nul 2>nul

REM Copy daemon + CLI into frontend/dist for Electron bundling
copy /Y "%OUT_DIR%\tamux-daemon.exe" "%PROJECT_ROOT%\frontend\dist\" >nul
copy /Y "%OUT_DIR%\tamux.exe"        "%PROJECT_ROOT%\frontend\dist\" >nul
if exist "%PROJECT_ROOT%\docs\getting-started.md" (
    copy /Y "%PROJECT_ROOT%\docs\getting-started.md" "%OUT_DIR%\GETTING_STARTED.md" >nul
    copy /Y "%PROJECT_ROOT%\docs\getting-started.md" "%PROJECT_ROOT%\frontend\dist\GETTING_STARTED.md" >nul
)
echo       Done.

REM -----------------------------------------------------------
REM Step 4: Sign Rust binaries (if requested)
REM -----------------------------------------------------------
if %SIGN%==1 (
    echo [4/5] Signing Rust binaries...
    call :sign_file "%OUT_DIR%\tamux-daemon.exe"
    call :sign_file "%OUT_DIR%\tamux.exe"
    if exist "%OUT_DIR%\tamux-mcp.exe"     call :sign_file "%OUT_DIR%\tamux-mcp.exe"
    if exist "%OUT_DIR%\tamux-gateway.exe"  call :sign_file "%OUT_DIR%\tamux-gateway.exe"
    echo       Done.
) else (
    echo [4/5] Skipping code signing (pass --sign to enable).
)

REM -----------------------------------------------------------
REM Step 5: Build Electron installer (portable + NSIS)
REM -----------------------------------------------------------
echo [5/5] Building Electron app...
cd /d "%PROJECT_ROOT%\frontend"
del /Q "%PROJECT_ROOT%\frontend\release\tamux*" >nul 2>nul
del /Q "%PROJECT_ROOT%\frontend\release\amux*" >nul 2>nul

if %SIGN%==1 (
    REM electron-builder reads CSC_LINK and CSC_KEY_PASSWORD for signing
    if defined TAMUX_SIGN_CERT (
        set CSC_LINK=%TAMUX_SIGN_CERT%
    ) else (
        if defined AMUX_SIGN_CERT (
        set CSC_LINK=%AMUX_SIGN_CERT%
        )
    )
    if defined TAMUX_SIGN_PASSWORD (
        set CSC_KEY_PASSWORD=%TAMUX_SIGN_PASSWORD%
    ) else (
        if defined AMUX_SIGN_PASSWORD (
        set CSC_KEY_PASSWORD=%AMUX_SIGN_PASSWORD%
        )
    )
)

call npx electron-builder --win portable nsis
if errorlevel 1 (
    echo ERROR: Electron build failed.
    exit /b 1
)

REM Copy Electron artifacts to dist-release
if exist "%PROJECT_ROOT%\frontend\release\tamux-portable.exe" (
    copy /Y "%PROJECT_ROOT%\frontend\release\tamux-portable.exe" "%OUT_DIR%\" >nul
)
for %%f in ("%PROJECT_ROOT%\frontend\release\tamux Setup*.exe") do (
    copy /Y "%%f" "%OUT_DIR%\" >nul
)
echo       Done.

REM -----------------------------------------------------------
REM Summary
REM -----------------------------------------------------------
echo.
echo ============================================================
echo  Build complete!
echo ============================================================
echo.
echo  Output:   %OUT_DIR%\
echo.
echo  Binaries:
echo    tamux.exe             CLI
echo    tamux-daemon.exe      Daemon
echo    tamux-mcp.exe         MCP server
echo    tamux-gateway.exe     Chat gateway
echo.
echo  Electron:
echo    tamux-portable.exe    Portable app
echo    tamux Setup *.exe     NSIS installer
echo.
if %SIGN%==0 (
    echo  NOTE: Binaries are NOT signed. Run with --sign to sign.
)
echo ============================================================

goto :eof

REM ============================================================
REM :sign_file  -- Sign a single executable with signtool
REM ============================================================
:sign_file
set FILE=%~1
if not exist "%FILE%" (
    echo       Skipping %FILE% (not found)
    goto :eof
)

where signtool >nul 2>nul
if errorlevel 1 (
    echo       WARNING: signtool.exe not found in PATH.
    echo       Install Windows SDK or Visual Studio Build Tools.
    goto :eof
)

if defined TAMUX_SIGN_CERT (
    REM PFX file signing
    signtool sign /f "%TAMUX_SIGN_CERT%" /p "%TAMUX_SIGN_PASSWORD%" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%FILE%"
    goto :eof
)

if defined AMUX_SIGN_CERT (
    REM Legacy PFX file signing
    signtool sign /f "%AMUX_SIGN_CERT%" /p "%AMUX_SIGN_PASSWORD%" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%FILE%"
    goto :eof
)

if defined TAMUX_SIGN_THUMBPRINT (
    REM Certificate store signing (hardware token / cert store)
    signtool sign /sha1 "%TAMUX_SIGN_THUMBPRINT%" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%FILE%"
    goto :eof
)

if defined AMUX_SIGN_THUMBPRINT (
    REM Legacy certificate store signing (hardware token / cert store)
    signtool sign /sha1 "%AMUX_SIGN_THUMBPRINT%" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%FILE%"
    goto :eof
)

echo       WARNING: No signing certificate configured.
echo       Set TAMUX_SIGN_CERT + TAMUX_SIGN_PASSWORD (PFX file)
echo       or TAMUX_SIGN_THUMBPRINT (certificate store).
