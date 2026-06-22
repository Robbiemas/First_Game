@echo off
setlocal

cd /d "%~dp0.."

set "RUNTIME_EXE=%CD%\target\release\mole_runtime.exe"

if not exist "%RUNTIME_EXE%" (
    echo Release runtime missing; building once...
    goto build_runtime
)

powershell -NoProfile -ExecutionPolicy Bypass -Command "$exe = '%RUNTIME_EXE%'; $exeTime = (Get-Item -LiteralPath $exe).LastWriteTimeUtc; $roots = @('Cargo.toml', 'Cargo.lock', 'crates\mole_core\Cargo.toml', 'crates\mole_core\src', 'crates\mole_runtime\Cargo.toml', 'crates\mole_runtime\build.rs', 'crates\mole_runtime\src'); $newer = Get-ChildItem -LiteralPath $roots -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTimeUtc -gt $exeTime } | Select-Object -First 1; if ($newer) { Write-Host ('Runtime source newer than release exe: ' + $newer.FullName); exit 2 }; exit 0"
set "STALE_CHECK=%ERRORLEVEL%"

if "%STALE_CHECK%"=="2" goto build_runtime
if not "%STALE_CHECK%"=="0" (
    echo Could not check release runtime freshness.
    exit /b %STALE_CHECK%
)

exit /b 0

:build_runtime
cargo build --release -p mole_runtime --features "sdl wup"
if errorlevel 1 (
    echo Mole Rust SDL3 runtime build failed.
    exit /b 1
)

exit /b 0
