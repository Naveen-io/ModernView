# ModernView Windows Installer Script
param(
    [switch]$SkipAssociations = $false,
    [switch]$OpenDefaultApps = $false
)

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "               ModernView Setup for Windows               " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

$installDir = "$env:LOCALAPPDATA\Programs\ModernView"
$sourceExe = Join-Path $PSScriptRoot "..\target\release\modernview.exe"

if (-not (Test-Path $sourceExe)) {
    $sourceExe = Join-Path $PSScriptRoot "modernview.exe"
}

if (-not (Test-Path $sourceExe)) {
    $rootRelease = Join-Path $PSScriptRoot "..\target\debug\modernview.exe"
    if (Test-Path $rootRelease) {
        $sourceExe = $rootRelease
    }
}

if (-not (Test-Path $sourceExe)) {
    Write-Host "[i] Executable not found. Checking if cargo is installed..." -ForegroundColor Yellow
    $cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
    if ($cargoCmd) {
        Write-Host "Building modernview in release mode..." -ForegroundColor Cyan
        Push-Location (Join-Path $PSScriptRoot "..")
        cargo build --release
        Pop-Location
        $sourceExe = Join-Path $PSScriptRoot "..\target\release\modernview.exe"
    }
}

if (-not (Test-Path $sourceExe)) {
    Write-Error "Could not find or build modernview.exe. Please build with 'cargo build --release' first."
    exit 1
}

Write-Host "Installing to: $installDir" -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

Copy-Item -Path $sourceExe -Destination "$installDir\modernview.exe" -Force
Write-Host "[✓] Copied modernview.exe" -ForegroundColor Green

# Create Start Menu Shortcut
$startMenuPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\ModernView.lnk"
$wshShell = New-Object -ComObject WScript.Shell
$shortcut = $wshShell.CreateShortcut($startMenuPath)
$shortcut.TargetPath = "$installDir\modernview.exe"
$shortcut.WorkingDirectory = $installDir
$shortcut.Description = "ModernView — Fast Modern Image Viewer & Editor"
$shortcut.Save()
Write-Host "[✓] Created Start Menu Shortcut" -ForegroundColor Green

# Create Desktop Shortcut
$desktopPath = [System.Environment]::GetFolderPath([System.Environment+SpecialFolder]::Desktop) + "\ModernView.lnk"
$deskShortcut = $wshShell.CreateShortcut($desktopPath)
$deskShortcut.TargetPath = "$installDir\modernview.exe"
$deskShortcut.WorkingDirectory = $installDir
$deskShortcut.Description = "ModernView — Fast Modern Image Viewer & Editor"
$deskShortcut.Save()
Write-Host "[✓] Created Desktop Shortcut" -ForegroundColor Green

# Register Windows Shell context menu and Default Apps
if (-not $SkipAssociations) {
    Write-Host "Registering Windows context menu and file associations..." -ForegroundColor Yellow
    $assocScript = Join-Path $PSScriptRoot "register_associations.ps1"
    if (Test-Path $assocScript) {
        & $assocScript -TargetExe "$installDir\modernview.exe" -OpenDefaultApps:$OpenDefaultApps
    }
}

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "Installation completed successfully!" -ForegroundColor Green
Write-Host "Launch ModernView from the Desktop or Start Menu." -ForegroundColor Cyan
Write-Host "Right-click any image in Windows Explorer to see 'Open with ModernView'!" -ForegroundColor Yellow
Write-Host "==========================================================" -ForegroundColor Green
