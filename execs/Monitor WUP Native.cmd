@echo off
setlocal
cd /d "%~dp0.."
call "%~dp0Setup SDL3.cmd" --no-pause
set "SDL3_ROOT=%CD%\.local\SDL3"
set "PATH=%USERPROFILE%\.cargo\bin;%SDL3_ROOT%\lib\x64;%PATH%"
cargo run -p mole_runtime --features "sdl wup" -- --monitor-wup
pause
