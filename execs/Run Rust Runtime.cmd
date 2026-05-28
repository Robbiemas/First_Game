@echo off
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo run -p mole_runtime -- --frames 120
pause
