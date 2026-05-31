@echo off
setlocal
cd /d "%~dp0..\.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo run -p mole_runtime --features wup -- --wup --frames 600
pause
