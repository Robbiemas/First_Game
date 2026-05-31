@echo off
setlocal

cd /d "%~dp0.."

set "PYTHON=%CD%\.venv\Scripts\python.exe"
set "VIEWER=%CD%\tools\state_graph_viewer.py"

if not exist "%PYTHON%" (
    echo Missing .venv\Scripts\python.exe. Run depreciated\Live Test Session.cmd once to set up Python.
    pause
    exit /b 1
)

if not exist "%VIEWER%" (
    echo Missing tools\state_graph_viewer.py.
    pause
    exit /b 1
)

"%PYTHON%" "%VIEWER%"
exit /b %ERRORLEVEL%
