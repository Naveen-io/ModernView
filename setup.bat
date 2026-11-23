@echo off
setlocal
title ModernView Setup

echo ==========================================================
echo               ModernView Setup for Windows               
echo ==========================================================
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\install.ps1"

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Setup encountered an issue.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo Press any key to finish...
pause >nul
