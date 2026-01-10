@echo off
echo Starting update...
powershell -ExecutionPolicy Bypass -File "%~dp0update.ps1"
pause
