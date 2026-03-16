#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$Check,
    [ValidateSet("source", "desktop")]
    [string]$Profile = "source",
    [ValidateSet("text", "json")]
    [string]$Format = "text"
)

$ErrorActionPreference = "Stop"

function Test-Tool($Name) {
    try {
        $cmd = Get-Command $Name -ErrorAction Stop
        return @{
            Found = $true
            Path = $cmd.Source
        }
    } catch {
        return @{
            Found = $false
            Path = ""
        }
    }
}

function Get-InstallHint($Dependency) {
    switch ($Dependency) {
        "cargo" { return "winget install Rustlang.Rustup" }
        "node" { return "winget install OpenJS.NodeJS.LTS" }
        "npm" { return "winget install OpenJS.NodeJS.LTS" }
        "git" { return "winget install Git.Git" }
        "uv" { return "powershell -ExecutionPolicy ByPass -c `"irm https://astral.sh/uv/install.ps1 | iex`"" }
        "aline" { return "uv tool install aline-ai" }
        "tamux-mcp" { return "cargo build --release -p tamux-mcp" }
        "hermes" { return "python -m pip install `"hermes-agent[all]`"" }
        "openclaw" { return "npm install -g openclaw" }
        default { return "No install hint available" }
    }
}

$requiredDeps = if ($Profile -eq "source") {
    @("cargo", "node", "npm", "git", "uv")
} else {
    @()
}

$optionalDeps = @("aline", "tamux-mcp", "hermes", "openclaw")

$requiredRows = @()
$optionalRows = @()
$missingRequired = @()

foreach ($dep in $requiredDeps) {
    $probe = Test-Tool $dep
    $row = [ordered]@{
        name = $dep
        found = [bool]$probe.Found
        path = $probe.Path
        install_hint = Get-InstallHint $dep
    }
    $requiredRows += $row
    if (-not $probe.Found) {
        $missingRequired += $dep
    }
}

foreach ($dep in $optionalDeps) {
    $probe = Test-Tool $dep
    $optionalRows += [ordered]@{
        name = $dep
        found = [bool]$probe.Found
        path = $probe.Path
        install_hint = Get-InstallHint $dep
    }
}

$report = [ordered]@{
    platform = "windows"
    profile = $Profile
    required = $requiredRows
    optional = $optionalRows
    missing_required = $missingRequired
}

if ($Format -eq "json") {
    $report | ConvertTo-Json -Depth 8
} else {
    Write-Host "tamux setup preflight"
    Write-Host "Profile:  $Profile"
    Write-Host "Platform: windows"
    Write-Host ""
    Write-Host "Required dependencies:"
    foreach ($row in $requiredRows) {
        if ($row.found) {
            Write-Host "  [ok]      $($row.name) ($($row.path))"
        } else {
            Write-Host "  [missing] $($row.name)"
            Write-Host "            install: $($row.install_hint)"
        }
    }
    Write-Host ""
    Write-Host "Optional dependencies:"
    foreach ($row in $optionalRows) {
        if ($row.found) {
            Write-Host "  [ok]               $($row.name) ($($row.path))"
        } else {
            Write-Host "  [optional-missing] $($row.name)"
            Write-Host "                     install: $($row.install_hint)"
        }
    }

    if ($missingRequired.Count -gt 0) {
        Write-Host ""
        Write-Host ("Missing required dependencies: {0}" -f ($missingRequired -join ", "))
    } else {
        Write-Host ""
        Write-Host "All required dependencies are installed."
    }
}

if ($Check -and $missingRequired.Count -gt 0) {
    exit 1
}

exit 0
