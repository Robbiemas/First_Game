@echo off
setlocal

cd /d "%~dp0..\.."

set "REQUIREMENTS=%CD%\requirements.txt"
set "PYTHON=%CD%\.venv\Scripts\python.exe"
set "WATCHER=%CD%\tools\live_test_session.py"

if not exist "%WATCHER%" (
    echo Missing tools\live_test_session.py.
    pause
    exit /b 1
)

if not exist "%PYTHON%" (
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

"%PYTHON%" "%WATCHER%" %*
set "LIVE_EXIT=%ERRORLEVEL%"

if not "%LIVE_EXIT%"=="0" (
    echo.
    echo Live test session exited with code %LIVE_EXIT%.
)

exit /b %LIVE_EXIT%
