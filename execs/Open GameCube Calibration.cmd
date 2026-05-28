@echo off
setlocal

cd /d "%~dp0.."

echo Manual endpoint calibration is deprecated.
echo Native WUP input now uses GameCube-style origin capture plus Rust/UCF input cleanup.
echo This file is kept only for old experiments and is not used by current gameplay.
echo.

if not exist "config\gamecube_calibration.json" (
    echo Missing config\gamecube_calibration.json
    pause
    exit /b 1
)

start "" notepad "config\gamecube_calibration.json"
