@echo off
setlocal

cd /d "%~dp0.."

set "REQUIREMENTS=%CD%\requirements.txt"
set "PYTHON=%CD%\.venv\Scripts\python.exe"
set "VIEWER=%CD%\tools\state_graph_viewer.py"

if not exist "%VIEWER%" (
    echo Missing tools\state_graph_viewer.py.
    pause
    exit /b 1
)

if not exist "%PYTHON%" (
    echo Setting up the local Python environment for the legacy parity ledger...
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

    if exist "%REQUIREMENTS%" (
        "%PYTHON%" -m pip install -r "%REQUIREMENTS%"
        if errorlevel 1 (
            echo Dependency installation failed.
            pause
            exit /b 1
        )
    )
)

"%PYTHON%" "%VIEWER%"
exit /b %ERRORLEVEL%
