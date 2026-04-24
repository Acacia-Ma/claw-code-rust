# install.ps1 — Download and install the latest devo binary for Windows.
#
# Usage (run as administrator is not required, installs to user-local bin):
#   irm https://raw.githubusercontent.com/7df-lab/devo/main/install.ps1 | iex
#
# Pin a specific version:
#   $env:VERSION = "v0.1.0"; irm https://raw.githubusercontent.com/7df-lab/devo/main/install.ps1 | iex

$Repo = "7df-lab/devo"

# ── Platform detection ───────────────────────────────────────────────────
function Get-Target {
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else {
        Write-Error "32-bit Windows is not supported"
        exit 1
    }
    return "${arch}-pc-windows-msvc"
}

# ── Resolve version ──────────────────────────────────────────────────────
function Resolve-Version {
    if ($env:VERSION) {
        return $env:VERSION
    }

    $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    return $latest.tag_name
}

# ── Install ──────────────────────────────────────────────────────────────
function Main {
    $target = Get-Target
    $version = Resolve-Version
    $archiveUrl = "https://github.com/$Repo/releases/download/$version/devo-${version}-${target}.zip"

    Write-Host "Downloading devo $version for $target ..."

    $tmpDir = Join-Path $env:TEMP "devo-install"
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue | Out-Null
    New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

    $zipPath = Join-Path $tmpDir "devo.zip"
    Invoke-WebRequest -Uri $archiveUrl -OutFile $zipPath

    Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force

    # Locate devo.exe (it's inside a versioned subdirectory).
    $exe = Get-ChildItem -Recurse -Filter "devo.exe" -Path $tmpDir | Select-Object -First 1
    if (-not $exe) {
        Write-Error "devo.exe not found in the archive"
        exit 1
    }

    # Install target.
    $installDir = Join-Path $env:LOCALAPPDATA "Programs\devo"
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item -Path $exe.FullName -Destination (Join-Path $installDir "devo.exe") -Force

    # Add to user PATH if not already present.
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$installDir*") {
        $newPath = $installDir + ";" + $currentPath
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        # Also update current session.
        $env:Path = $installDir + ";" + $env:Path
    }

    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue | Out-Null

    Write-Host "Installed devo to ${installDir}\devo.exe"
    Write-Host "Run 'devo onboard' to get started."
}

Main
