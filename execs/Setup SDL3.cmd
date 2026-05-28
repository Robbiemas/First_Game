@echo off
setlocal
cd /d "%~dp0.."
powershell -NoProfile -ExecutionPolicy Bypass -File "tools\setup_sdl3.ps1"
if /I not "%~1"=="--no-pause" pause
