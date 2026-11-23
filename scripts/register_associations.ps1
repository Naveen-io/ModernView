# ModernView Shell & Association Registration Script
# Run this script to register ModernView in Windows Explorer right-click context menu,
# "Open with" submenu, and Windows Settings > Apps > Default Apps.

param(
    [string]$TargetExe = ""
)

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "       ModernView Windows Shell Integration Setup         " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

if (-not $TargetExe) {
    # Check default install path first
    $installed = "$env:LOCALAPPDATA\Programs\ModernView\modernview.exe"
    $devRelease = Join-Path $PSScriptRoot "..\target\release\modernview.exe"

    if (Test-Path $installed) {
        $TargetExe = (Resolve-Path $installed).Path
    } elseif (Test-Path $devRelease) {
        $TargetExe = (Resolve-Path $devRelease).Path
    } else {
        $localExe = Join-Path $PSScriptRoot "modernview.exe"
        if (Test-Path $localExe) {
            $TargetExe = (Resolve-Path $localExe).Path
        }
    }
}

if (-not $TargetExe -or -not (Test-Path $TargetExe)) {
    Write-Error "Could not locate modernview.exe. Build with 'cargo build --release' or specify -TargetExe 'C:\path\to\modernview.exe'."
    exit 1
}

$escapedExe = $TargetExe.Replace("\", "\\")
Write-Host "Registering ModernView executable: $TargetExe" -ForegroundColor Yellow

$regContent = @"
Windows Registry Editor Version 5.00

; ModernView Application Handler
[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe]
@="ModernView"
"FriendlyAppName"="ModernView"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\DefaultIcon]
@="\"$escapedExe\",0"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\shell\open]
@="Open with ModernView"
"FriendlyAppName"="ModernView"

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\shell\open\command]
@="\"$escapedExe\" \"%1\""

[HKEY_CURRENT_USER\Software\Classes\Applications\modernview.exe\SupportedTypes]
".jpg"=""
".jpeg"=""
".png"=""
".webp"=""
".bmp"=""
".gif"=""
".tiff"=""
".tif"=""
".ico"=""

; ProgID Registration
[HKEY_CURRENT_USER\Software\Classes\ModernView.Image]
@="ModernView Image Document"
"FriendlyTypeName"="ModernView Image"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\DefaultIcon]
@="\"$escapedExe\",0"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\shell\open]
@="Open with ModernView"

[HKEY_CURRENT_USER\Software\Classes\ModernView.Image\shell\open\command]
@="\"$escapedExe\" \"%1\""

; Windows Explorer Context Menu for Images (SystemFileAssociations)
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\image\shell\ModernView]
@="Open with ModernView"
"Icon"="\"$escapedExe\",0"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\image\shell\ModernView\command]
@="\"$escapedExe\" \"%1\""

; Extensions OpenWithProgids Registration
[HKEY_CURRENT_USER\Software\Classes\.jpg\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.jpeg\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.png\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.webp\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.bmp\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.gif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.tiff\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.tif\OpenWithProgids]
"ModernView.Image"=""

[HKEY_CURRENT_USER\Software\Classes\.ico\OpenWithProgids]
"ModernView.Image"=""

; Windows Default Apps Integration
[HKEY_CURRENT_USER\Software\RegisteredApplications]
"ModernView"="Software\\ModernView\\Capabilities"

[HKEY_CURRENT_USER\Software\ModernView\Capabilities]
"ApplicationName"="ModernView"
"ApplicationDescription"="Fast, Modern Image Viewer & Editor for Windows"

[HKEY_CURRENT_USER\Software\ModernView\Capabilities\FileAssociations]
".jpg"="ModernView.Image"
".jpeg"="ModernView.Image"
".png"="ModernView.Image"
".webp"="ModernView.Image"
".bmp"="ModernView.Image"
".gif"="ModernView.Image"
".tiff"="ModernView.Image"
".tif"="ModernView.Image"
".ico"="ModernView.Image"
"@

$tempRegPath = Join-Path $env:TEMP "modernview_register.reg"
[System.IO.File]::WriteAllText($tempRegPath, $regContent, [System.Text.Encoding]::Unicode)

Write-Host "Importing registry keys..." -ForegroundColor Yellow
$proc = Start-Process -FilePath "reg.exe" -ArgumentList "import `"$tempRegPath`"" -Wait -PassThru -NoNewWindow
if ($proc.ExitCode -eq 0) {
    Write-Host "[✓] Registry keys imported successfully into HKCU." -ForegroundColor Green
} else {
    Write-Error "Failed to import registry keys (exit code $($proc.ExitCode))."
    exit 1
}

# Clean up temp file
Remove-Item -Force $tempRegPath -ErrorAction SilentlyContinue

# Notify Windows Explorer Shell of file association update
Write-Host "Notifying Windows Shell of association changes..." -ForegroundColor Yellow
$notifyCode = @'
using System;
using System.Runtime.InteropServices;
public class ShellNotifier {
    [DllImport("shell32.dll")]
    public static extern void SHChangeNotify(int wEventId, int uFlags, IntPtr dwItem1, IntPtr dwItem2);
    public static void Notify() {
        SHChangeNotify(0x08000000, 0, IntPtr.Zero, IntPtr.Zero);
    }
}
'@
try {
    Add-Type -TypeDefinition $notifyCode -Language CSharp
    [ShellNotifier]::Notify()
    Write-Host "[✓] Windows Explorer notified." -ForegroundColor Green
} catch {
    # Non-fatal fallback
}

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "✓ ModernView is now registered in Windows:" -ForegroundColor Green
Write-Host "  1. Right-click any image -> 'Open with ModernView' is available" -ForegroundColor Cyan
Write-Host "  2. Right-click -> 'Open with' submenu lists ModernView" -ForegroundColor Cyan
Write-Host "  3. Windows Settings > Apps > Default Apps lists ModernView" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Green
