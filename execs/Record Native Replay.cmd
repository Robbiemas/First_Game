@echo off
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo run -p mole_runtime -- --record-replay --frames 600
pause
