# Screen Time Manager - Install Script
# Run as Administrator for best results

param(
    [string]$ExePath = ""
)

$ErrorActionPreference = "Stop"
$TaskName = "ScreenTimeManager"
$Description = "Screen Time Manager - Manages daily computer time limits"

Write-Host "Screen Time Manager - Installation" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan
Write-Host ""

# Find the executable
if ($ExePath -eq "") {
    # Try to find it in common locations
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $possiblePaths = @(
        (Join-Path $scriptDir "screen-time-manager.exe"),
        (Join-Path $scriptDir "target\release\screen-time-manager.exe"),
        (Join-Path $scriptDir "target\debug\screen-time-manager.exe")
    )

    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $ExePath = $path
            break
        }
    }
}

if ($ExePath -eq "" -or -not (Test-Path $ExePath)) {
    Write-Host "ERROR: Could not find screen-time-manager.exe" -ForegroundColor Red
    Write-Host ""
    Write-Host "Usage: .\install.ps1 -ExePath 'C:\path\to\screen-time-manager.exe'" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

$ExePath = (Resolve-Path $ExePath).Path
Write-Host "Found executable: $ExePath" -ForegroundColor Green
Write-Host ""

# Check if task already exists
$existingTask = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($existingTask) {
    Write-Host "Existing installation found. Removing..." -ForegroundColor Yellow
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
}

# Create the scheduled task
Write-Host "Creating scheduled task..." -ForegroundColor White

$action = New-ScheduledTaskAction -Execute $ExePath
$trigger = New-ScheduledTaskTrigger -AtLogOn
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -ExecutionTimeLimit (New-TimeSpan -Days 0) `
    -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1)

$task = New-ScheduledTask -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description $Description

Register-ScheduledTask -TaskName $TaskName -InputObject $task | Out-Null

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Screen Time Manager will now start automatically when you log in." -ForegroundColor White
Write-Host ""

# Ask if user wants to start it now
$response = Read-Host "Do you want to start Screen Time Manager now? (Y/n)"
if ($response -eq "" -or $response -match "^[Yy]") {
    Write-Host "Starting Screen Time Manager..." -ForegroundColor White
    Start-ScheduledTask -TaskName $TaskName
    Write-Host "Started!" -ForegroundColor Green
}

Write-Host ""
Write-Host "To uninstall, run: .\uninstall.ps1" -ForegroundColor Cyan
Write-Host ""
