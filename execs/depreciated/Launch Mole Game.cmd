@echo off
setlocal

cd /d "%~dp0..\.."

set "GAME=%CD%\RealMainFile.py"
set "REQUIREMENTS=%CD%\requirements.txt"
set "PYTHON=%CD%\.venv\Scripts\python.exe"
set "PYTHONW=%CD%\.venv\Scripts\pythonw.exe"
set "WUP_TARGET=%CD%\target-wup"
set "MOLE_RUNTIME_EXE=%WUP_TARGET%\debug\mole_runtime.exe"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "SDL_JOYSTICK_HIDAPI=1"
set "SDL_JOYSTICK_HIDAPI_GAMECUBE=1"

if /I "%~1"=="--check" goto check

if not exist "%GAME%" (
    echo Could not find RealMainFile.py in the project root.
    pause
    exit /b 1
)

if not exist "%PYTHONW%" (
    echo Setting up the local Python environment...
    where py >nul 2>nul
    if errorlevel 1 (
        echo Python launcher "py" was not found. Install Python 3.10, then try again.
        pause
        exit /b 1
    )

    py -3.10 -m venv .venv
    if errorlevel 1 (
        echo Could not create .venv with Python 3.10.
        pause
        exit /b 1
    )

    "%PYTHON%" -m pip install -r "%REQUIREMENTS%"
    if errorlevel 1 (
        echo Dependency installation failed.
        pause
        exit /b 1
    )
)

where cargo >nul 2>nul
if not errorlevel 1 (
    cargo build -q -p mole_runtime --features wup --target-dir "%WUP_TARGET%" >nul 2>nul
)

if "%MOLE_DEBUG_INPUTS%"=="1" (
    "%PYTHON%" "%GAME%"
    set "GAME_EXIT=%ERRORLEVEL%"
    if not "%GAME_EXIT%"=="0" (
        echo.
        echo Mole Game exited with an error. Check the newest logs\crash-*.log file.
    )
    pause
    exit /b %GAME_EXIT%
)

start "Mole Game" "%PYTHONW%" "%GAME%"
exit /b 0

:check
if not exist "%GAME%" (
    echo Missing RealMainFile.py
    exit /b 1
)

if not exist "%PYTHONW%" (
    echo Missing .venv Python runtime
    exit /b 1
)

echo Launcher check passed.
exit /b 0
