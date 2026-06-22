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
call "%~dp0Ensure Release Runtime.cmd"
if errorlevel 1 (
    pause
    exit /b 1
)
echo Launching local Mole Rust SDL3 runtime with native WUP controller input...
"%RUNTIME_EXE%" --sdl --play --input-trace
if errorlevel 1 (
    echo Mole Rust SDL3 runtime exited with an error.
    pause
    exit /b 1
)
pause
