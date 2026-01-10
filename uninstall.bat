@echo off
echo Starting uninstallation...
powershell -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
pause
