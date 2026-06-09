@echo off
setlocal
cd /d "%~dp0.."
call "%~dp0Setup SDL3.cmd" --no-pause
if errorlevel 1 (
    echo SDL3 setup failed.
    pause
    exit /b 1
)
set "SDL3_ROOT=%CD%\.local\SDL3"
set "PATH=%USERPROFILE%\.cargo\bin;%SDL3_ROOT%\lib\x64;%PATH%"
if exist "%~dp0Open State Graphs.cmd" (
    start "Mole Game Dev Tool" "%~dp0Open State Graphs.cmd"
)
echo Launching Mole Rust SDL3 local runtime...
call cargo run --release -p mole_runtime --features "sdl wup" -- --sdl --play --input-trace
if errorlevel 1 (
    echo Mole Rust SDL3 runtime exited with an error.
    pause
    exit /b 1
)
pause
