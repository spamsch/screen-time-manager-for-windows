# Screen Time Manager - Uninstall Script

$ErrorActionPreference = "Stop"
$TaskName = "ScreenTimeManager"

Write-Host "Screen Time Manager - Uninstallation" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Check if task exists
$existingTask = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if (-not $existingTask) {
    Write-Host "Screen Time Manager is not installed (no scheduled task found)." -ForegroundColor Yellow
    Write-Host ""
    exit 0
}

# Stop if running
Write-Host "Stopping Screen Time Manager if running..." -ForegroundColor White
Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue

# Also kill the process if it's running
$process = Get-Process -Name "screen-time-manager" -ErrorAction SilentlyContinue
if ($process) {
    Write-Host "Terminating running process..." -ForegroundColor White
    Stop-Process -Name "screen-time-manager" -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

# Remove the scheduled task
Write-Host "Removing scheduled task..." -ForegroundColor White
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false

Write-Host ""
Write-Host "Uninstallation complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Screen Time Manager will no longer start automatically." -ForegroundColor White
Write-Host ""
Write-Host "Note: The executable and database files have not been deleted." -ForegroundColor Yellow
Write-Host "To completely remove all data, delete the following folder:" -ForegroundColor Yellow
Write-Host "  $env:APPDATA\ScreenTimeManager" -ForegroundColor Cyan
Write-Host ""
