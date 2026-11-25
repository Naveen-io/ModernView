@echo off
setlocal
title ModernView Uninstaller

echo ==========================================================
echo              ModernView Uninstaller for Windows            
echo ==========================================================
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\uninstall.ps1"

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Uninstall encountered an issue.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo Press any key to finish...
pause >nul
