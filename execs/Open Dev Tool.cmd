@echo off
setlocal

cd /d "%~dp0.."

set "CARGO=cargo"
set "DEVTOOL_MANIFEST=%CD%\Cargo.toml"

where %CARGO% >nul 2>nul
if errorlevel 1 (
    echo Missing cargo on PATH. Install Rust or open the devtool from a Rust-enabled shell.
    pause
    exit /b 1
)

if not exist "%DEVTOOL_MANIFEST%" (
    echo Missing Cargo.toml at the repository root.
    pause
    exit /b 1
)

%CARGO% run -p mole_devtool --manifest-path "%DEVTOOL_MANIFEST%"
exit /b %ERRORLEVEL%
