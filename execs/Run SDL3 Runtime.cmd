@echo off
setlocal
cd /d "%~dp0.."
set "SDL3_ROOT=%CD%\.local\SDL3"
set "RUNTIME_EXE=%CD%\target\release\mole_runtime.exe"
if not exist "%SDL3_ROOT%\lib\x64\SDL3.dll" (
    call "%~dp0Setup SDL3.cmd" --no-pause
    if errorlevel 1 (
        echo SDL3 setup failed.
        pause
        exit /b 1
    )
)
set "PATH=%USERPROFILE%\.cargo\bin;%SDL3_ROOT%\lib\x64;%PATH%"
if not exist "%RUNTIME_EXE%" (
    echo Release runtime missing; building once...
    call cargo build --release -p mole_runtime --features "sdl wup"
    if errorlevel 1 (
        echo Mole Rust SDL3 runtime build failed.
        pause
        exit /b 1
    )
)
if exist "%~dp0Open Dev Tool.cmd" (
    start "Mole Game Dev Tool" "%~dp0Open Dev Tool.cmd"
)
echo Launching Mole Rust SDL3 local runtime...
"%RUNTIME_EXE%" --sdl --play --input-trace
if errorlevel 1 (
    echo Mole Rust SDL3 runtime exited with an error.
    pause
    exit /b 1
)
pause
