#Requires -Version 5.1
<#
.SYNOPSIS
    tamux release build for Windows (PowerShell)

.DESCRIPTION
    Builds Rust binaries, frontend, and Electron app with optional code signing.
    Supports PFX file signing, certificate store signing, and Azure SignTool.

.PARAMETER Sign
    Enable code signing of all executables.

.PARAMETER SignTool
    Signing method: "signtool" (default), "azure", or "pfx".
    - signtool: Uses certificate thumbprint from store
    - pfx: Uses PFX file (set AMUX_SIGN_CERT and AMUX_SIGN_PASSWORD)
    - azure: Uses Azure Trusted Signing (set AZURE_* env vars)

.PARAMETER Thumbprint
    Certificate thumbprint for signtool signing. Overrides AMUX_SIGN_THUMBPRINT env var.

.PARAMETER CertFile
    Path to PFX certificate file. Overrides AMUX_SIGN_CERT env var.

.PARAMETER CertPassword
    PFX certificate password. Overrides AMUX_SIGN_PASSWORD env var.

.PARAMETER SkipRust
    Skip Rust compilation (use existing binaries).

.PARAMETER SkipFrontend
    Skip frontend build (use existing dist/).

.PARAMETER SkipElectron
    Skip Electron packaging.

.PARAMETER Target
    Rust target triple. Default: native.

.EXAMPLE
    .\scripts\build-release.ps1
    Build without signing.

.EXAMPLE
    .\scripts\build-release.ps1 -Sign -SignTool pfx -CertFile .\cert.pfx -CertPassword "secret"
    Build and sign with a PFX certificate.

.EXAMPLE
    .\scripts\build-release.ps1 -Sign -Thumbprint "ABCD1234..."
    Build and sign with certificate from Windows certificate store.
#>

[CmdletBinding()]
param(
    [switch]$Sign,
    [ValidateSet("signtool", "azure", "pfx")]
    [string]$SignTool = "signtool",
    [string]$Thumbprint,
    [string]$CertFile,
    [string]$CertPassword,
    [switch]$SkipRust,
    [switch]$SkipFrontend,
    [switch]$SkipElectron,
    [string]$Target
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$OutDir = Join-Path $ProjectRoot "dist-release"
$FrontendDir = Join-Path $ProjectRoot "frontend"

# ─────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────

function Write-Step($num, $total, $msg) {
    Write-Host "`n[$num/$total] $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "  $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "  WARNING: $msg" -ForegroundColor Yellow
}

function Sign-Binary([string]$FilePath) {
    if (-not (Test-Path $FilePath)) {
        Write-Warn "Skipping $FilePath (not found)"
        return
    }

    $fileName = Split-Path $FilePath -Leaf

    switch ($SignTool) {
        "pfx" {
            $cert = if ($CertFile) { $CertFile } else { $env:AMUX_SIGN_CERT }
            $pass = if ($CertPassword) { $CertPassword } else { $env:AMUX_SIGN_PASSWORD }
            if (-not $cert) {
                Write-Warn "No PFX certificate. Set -CertFile or AMUX_SIGN_CERT."
                return
            }
            Write-Host "  Signing $fileName (PFX)..."
            & signtool sign /f $cert /p $pass /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $FilePath
            if ($LASTEXITCODE -ne 0) { throw "signtool failed for $fileName" }
        }
        "signtool" {
            $thumb = if ($Thumbprint) { $Thumbprint } else { $env:AMUX_SIGN_THUMBPRINT }
            if (-not $thumb) {
                Write-Warn "No certificate thumbprint. Set -Thumbprint or AMUX_SIGN_THUMBPRINT."
                return
            }
            Write-Host "  Signing $fileName (cert store)..."
            & signtool sign /sha1 $thumb /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $FilePath
            if ($LASTEXITCODE -ne 0) { throw "signtool failed for $fileName" }
        }
        "azure" {
            # Azure Trusted Signing via AzureSignTool
            # Requires: dotnet tool install -g AzureSignTool
            $endpoint  = $env:AZURE_SIGN_ENDPOINT
            $account   = $env:AZURE_SIGN_ACCOUNT
            $certProf  = $env:AZURE_SIGN_CERT_PROFILE
            $tenantId  = $env:AZURE_TENANT_ID
            $clientId  = $env:AZURE_CLIENT_ID
            $clientSec = $env:AZURE_CLIENT_SECRET

            if (-not ($endpoint -and $account -and $certProf)) {
                Write-Warn "Azure Trusted Signing not configured. Set AZURE_SIGN_* env vars."
                return
            }
            Write-Host "  Signing $fileName (Azure Trusted Signing)..."
            & AzureSignTool sign `
                -kvu $endpoint `
                -kva $account `
                -kvt $tenantId `
                -kvi $clientId `
                -kvs $clientSec `
                -kvc $certProf `
                -tr http://timestamp.digicert.com `
                -td SHA256 `
                $FilePath
            if ($LASTEXITCODE -ne 0) { throw "AzureSignTool failed for $fileName" }
        }
    }
    Write-Ok "Signed $fileName"
}

function Verify-Signature([string]$FilePath) {
    if (-not (Test-Path $FilePath)) { return }
    $sig = Get-AuthenticodeSignature $FilePath
    $fileName = Split-Path $FilePath -Leaf
    if ($sig.Status -eq "Valid") {
        Write-Ok "$fileName signature: Valid ($($sig.SignerCertificate.Subject))"
    } else {
        Write-Warn "$fileName signature: $($sig.Status)"
    }
}

# ─────────────────────────────────────────────────────
# Build steps
# ─────────────────────────────────────────────────────

$totalSteps = 5
$step = 0

Write-Host "`n============================================================" -ForegroundColor White
Write-Host " tamux release build" -ForegroundColor White
Write-Host "============================================================" -ForegroundColor White

# Step 1: Rust
$step++
if ($SkipRust) {
    Write-Step $step $totalSteps "Skipping Rust build (--SkipRust)"
} else {
    Write-Step $step $totalSteps "Building Rust binaries (release)..."
    Push-Location $ProjectRoot
    try {
        $cargoArgs = @("build", "--release")
        if ($Target) { $cargoArgs += @("--target", $Target) }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }
        Write-Ok "Rust build complete"
    } finally {
        Pop-Location
    }
}

# Step 2: Frontend
$step++
if ($SkipFrontend) {
    Write-Step $step $totalSteps "Skipping frontend build (--SkipFrontend)"
} else {
    Write-Step $step $totalSteps "Building frontend..."
    Push-Location $FrontendDir
    try {
        & npm ci --silent 2>$null
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
        Write-Ok "Frontend build complete"
    } finally {
        Pop-Location
    }
}

# Step 3: Collect artifacts
$step++
Write-Step $step $totalSteps "Collecting artifacts..."
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$targetDir = if ($Target) { Join-Path $ProjectRoot "target" $Target "release" } else { Join-Path $ProjectRoot "target" "release" }

$binaries = @(
    @{ Name = "tamux-daemon"; Desc = "Daemon" },
    @{ Name = "tamux";        Desc = "CLI" },
    @{ Name = "tamux-mcp";    Desc = "MCP server" },
    @{ Name = "tamux-gateway"; Desc = "Chat gateway" }
)

foreach ($bin in $binaries) {
    $exe = Join-Path $targetDir "$($bin.Name).exe"
    if (Test-Path $exe) {
        Copy-Item $exe $OutDir -Force
        Write-Ok "$($bin.Desc): $($bin.Name).exe"
    } else {
        # Try without .exe (Linux build on WSL)
        $nix = Join-Path $targetDir $bin.Name
        if (Test-Path $nix) {
            Copy-Item $nix $OutDir -Force
            Write-Ok "$($bin.Desc): $($bin.Name)"
        }
    }
}

# Copy to frontend/dist for Electron bundling
$distDir = Join-Path $FrontendDir "dist"
if (Test-Path $distDir) {
    foreach ($name in @("tamux-daemon.exe", "tamux.exe")) {
        $src = Join-Path $OutDir $name
        if (Test-Path $src) { Copy-Item $src $distDir -Force }
    }
}

# Step 4: Code signing
$step++
if ($Sign) {
    Write-Step $step $totalSteps "Signing binaries..."
    Get-ChildItem $OutDir -Filter "*.exe" | ForEach-Object {
        Sign-Binary $_.FullName
    }
    Write-Ok "Signing complete"
} else {
    Write-Step $step $totalSteps "Skipping code signing (use -Sign to enable)"
}

# Step 5: Electron
$step++
if ($SkipElectron) {
    Write-Step $step $totalSteps "Skipping Electron build (--SkipElectron)"
} else {
    Write-Step $step $totalSteps "Building Electron app..."
    Push-Location $FrontendDir
    try {
        # Set signing env vars for electron-builder
        if ($Sign) {
            if ($CertFile -or $env:AMUX_SIGN_CERT) {
                $env:CSC_LINK = if ($CertFile) { $CertFile } else { $env:AMUX_SIGN_CERT }
                $env:CSC_KEY_PASSWORD = if ($CertPassword) { $CertPassword } else { $env:AMUX_SIGN_PASSWORD }
            }
            if ($Thumbprint -or $env:AMUX_SIGN_THUMBPRINT) {
                # For electron-builder custom signing, we need a sign hook
                $env:AMUX_SIGN_THUMBPRINT = if ($Thumbprint) { $Thumbprint } else { $env:AMUX_SIGN_THUMBPRINT }
            }
        }

        & npx electron-builder --win portable nsis
        if ($LASTEXITCODE -ne 0) { throw "Electron build failed" }

        # Copy Electron artifacts
        $releaseDir = Join-Path $FrontendDir "release"
        Get-ChildItem $releaseDir -Filter "*.exe" -ErrorAction SilentlyContinue | ForEach-Object {
            Copy-Item $_.FullName $OutDir -Force
            Write-Ok "Electron: $($_.Name)"
        }
    } finally {
        Pop-Location
    }
}

# ─────────────────────────────────────────────────────
# Verify signatures
# ─────────────────────────────────────────────────────
if ($Sign) {
    Write-Host "`nVerifying signatures..." -ForegroundColor Cyan
    Get-ChildItem $OutDir -Filter "*.exe" | ForEach-Object {
        Verify-Signature $_.FullName
    }
}

# ─────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────
Write-Host "`n============================================================" -ForegroundColor White
Write-Host " Build complete!" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor White
Write-Host ""
Write-Host "  Output: $OutDir" -ForegroundColor White
Write-Host ""
Get-ChildItem $OutDir | ForEach-Object {
    $size = "{0:N1} MB" -f ($_.Length / 1MB)
    Write-Host ("  {0,-30} {1}" -f $_.Name, $size) -ForegroundColor Gray
}
Write-Host ""

if (-not $Sign) {
    Write-Host "  Binaries are NOT signed. Run with -Sign to sign." -ForegroundColor Yellow
}
Write-Host "============================================================" -ForegroundColor White
