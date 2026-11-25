# ModernView Windows Uninstaller Script
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "              ModernView Uninstaller for Windows          " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

$installDir = "$env:LOCALAPPDATA\Programs\ModernView"
$startMenuPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\ModernView.lnk"
$desktopPath = [System.Environment]::GetFolderPath([System.Environment+SpecialFolder]::Desktop) + "\ModernView.lnk"

# Remove shortcuts
if (Test-Path $startMenuPath) {
    Remove-Item -Force $startMenuPath
    Write-Host "[✓] Removed Start Menu Shortcut" -ForegroundColor Green
}

if (Test-Path $desktopPath) {
    Remove-Item -Force $desktopPath
    Write-Host "[✓] Removed Desktop Shortcut" -ForegroundColor Green
}

# Remove Registry Entries
Write-Host "Removing registry associations..." -ForegroundColor Yellow
$keysToRemove = @(
    "HKCU:\Software\Classes\Applications\modernview.exe",
    "HKCU:\Software\Classes\ModernView.Image",
    "HKCU:\Software\Classes\SystemFileAssociations\image\shell\ModernView",
    "HKCU:\Software\ModernView"
)

foreach ($key in $keysToRemove) {
    if (Test-Path $key) {
        Remove-Item -Recurse -Force $key -ErrorAction SilentlyContinue
    }
}

# Remove from RegisteredApplications
Remove-ItemProperty -Path "HKCU:\Software\RegisteredApplications" -Name "ModernView" -ErrorAction SilentlyContinue

# Remove from OpenWithProgids
$exts = @(".jpg", ".jpeg", ".png", ".webp", ".bmp", ".gif", ".tiff", ".tif", ".ico", ".avif", ".heic", ".heif", ".qoi", ".tga", ".hdr")
foreach ($ext in $exts) {
    Remove-ItemProperty -Path "HKCU:\Software\Classes\$ext\OpenWithProgids" -Name "ModernView.Image" -ErrorAction SilentlyContinue
}

# Remove program folder
if (Test-Path $installDir) {
    Remove-Item -Recurse -Force $installDir
    Write-Host "[✓] Removed ModernView installation directory" -ForegroundColor Green
}

Write-Host ""
Write-Host "ModernView has been cleanly uninstalled from this computer." -ForegroundColor Green
