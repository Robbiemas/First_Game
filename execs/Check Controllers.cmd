@echo off
setlocal

cd /d "%~dp0.."

set "PYTHON=%CD%\.venv\Scripts\python.exe"
set "CHECKER=%CD%\tools\check_controllers.py"

set "SDL_JOYSTICK_HIDAPI=1"
set "SDL_JOYSTICK_HIDAPI_GAMECUBE=1"

if not exist "%PYTHON%" (
    echo Missing .venv Python runtime. Double-click execs\Launch Mole Game.cmd once to set it up.
    pause
    exit /b 1
)

if not exist "%CHECKER%" (
    echo Missing tools\check_controllers.py.
    pause
    exit /b 1
)

"%PYTHON%" "%CHECKER%" %*
pause
