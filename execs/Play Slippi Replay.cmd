@echo off
setlocal

cd /d "%~dp0.."

set "REPLAY_PATH=%CD%\replays\Game_20260530T214929.slp"
set "DIVERGENCE_LOG=%CD%\debug\slippi\runtime-divergence.latest.json"
set "LAUNCH_LOG=%CD%\debug\slippi\runtime-launch.latest.log"
set "SDL3_ROOT=%CD%\.local\SDL3"
set "RUNTIME_EXE=%CD%\target\release\mole_runtime.exe"

if not exist "%CD%\debug\slippi" mkdir "%CD%\debug\slippi"

echo Mole Slippi replay launch > "%LAUNCH_LOG%"
echo Replay: %REPLAY_PATH% >> "%LAUNCH_LOG%"
echo Divergence log: %DIVERGENCE_LOG% >> "%LAUNCH_LOG%"

if not exist "%REPLAY_PATH%" (
    echo Missing replay: %REPLAY_PATH% >> "%LAUNCH_LOG%"
    exit /b 1
)

if exist "%DIVERGENCE_LOG%" del /q "%DIVERGENCE_LOG%" >nul 2>nul

if not exist "%SDL3_ROOT%\lib\x64\SDL3.dll" (
    echo SDL3 runtime not found; running setup... >> "%LAUNCH_LOG%"
    call "%~dp0Setup SDL3.cmd" --no-pause >> "%LAUNCH_LOG%" 2>&1
    if errorlevel 1 (
        echo SDL3 setup failed. >> "%LAUNCH_LOG%"
        exit /b 1
    )
) else (
    echo SDL3 already available: %SDL3_ROOT%\lib\x64 >> "%LAUNCH_LOG%"
)

set "PATH=%USERPROFILE%\.cargo\bin;%SDL3_ROOT%\lib\x64;%PATH%"

call "%~dp0Ensure Release Runtime.cmd" >> "%LAUNCH_LOG%" 2>&1
if errorlevel 1 (
    echo Runtime build failed. >> "%LAUNCH_LOG%"
    exit /b 1
)

echo Using fresh release runtime: %RUNTIME_EXE% >> "%LAUNCH_LOG%"
echo Launching Mole Rust SDL3 Slippi replay runtime... >> "%LAUNCH_LOG%"
"%RUNTIME_EXE%" --sdl --visual-slippi-replay "%REPLAY_PATH%" --frames 4294967295 --slippi-divergence-log "%DIVERGENCE_LOG%" --hold-final-frame >> "%LAUNCH_LOG%" 2>&1
set "EXITCODE=%ERRORLEVEL%"

echo Runtime exited with %EXITCODE%. >> "%LAUNCH_LOG%"
if exist "%DIVERGENCE_LOG%" powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Summarize Divergence Log.ps1" "%DIVERGENCE_LOG%" >> "%LAUNCH_LOG%" 2>&1
exit /b %EXITCODE%
