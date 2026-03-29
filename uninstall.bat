@echo off

:: Check for admin privileges
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

echo Starting uninstallation...
powershell -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
pause
