$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$distRoot = Join-Path $repoRoot "dist"
$playtestRoot = Join-Path $repoRoot "playtest"
$packageName = "MoleGame-FriendPlaytest"
$packageRoot = Join-Path $distRoot $packageName
$zipPath = Join-Path $distRoot "$packageName.zip"
$bootstrapExe = Join-Path $distRoot "$packageName.exe"
$playtestExe = Join-Path $playtestRoot "$packageName.exe"

function Remove-PathInsideDist {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $resolvedDist = Resolve-Path -LiteralPath $distRoot
    $resolvedPath = Resolve-Path -LiteralPath $Path
    if (-not $resolvedPath.Path.StartsWith($resolvedDist.Path, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove path outside dist/: $($resolvedPath.Path)"
    }

    Remove-Item -LiteralPath $resolvedPath.Path -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
New-Item -ItemType Directory -Force -Path $playtestRoot | Out-Null
Remove-PathInsideDist -Path $packageRoot
Remove-PathInsideDist -Path $zipPath
Remove-PathInsideDist -Path $bootstrapExe
New-Item -ItemType Directory -Force -Path $packageRoot | Out-Null

Push-Location $repoRoot
try {
    & cargo build -p mole_runtime --release --features "sdl wup"
    & (Join-Path $repoRoot "tools\setup_sdl3.ps1")
} finally {
    Pop-Location
}

$runtimeExe = Join-Path $repoRoot "target\release\mole_runtime.exe"
$sdlDll = Join-Path $repoRoot ".local\SDL3\lib\x64\SDL3.dll"

if (-not (Test-Path -LiteralPath $runtimeExe)) {
    throw "mole_runtime.exe was not built at $runtimeExe"
}
if (-not (Test-Path -LiteralPath $sdlDll)) {
    throw "SDL3.dll was not found at $sdlDll"
}

Copy-Item -LiteralPath $runtimeExe -Destination (Join-Path $packageRoot "mole_runtime.exe") -Force
Copy-Item -LiteralPath $sdlDll -Destination (Join-Path $packageRoot "SDL3.dll") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "background.png") -Destination $packageRoot -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "DolphinMole") -Destination $packageRoot -Recurse -Force

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
mole_runtime.exe --sdl --play --input-trace
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Mole Game.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
echo Running vanilla pre-UCF input mode.
mole_runtime.exe --sdl --play --input-trace --no-ucf
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Mole Game Vanilla No UCF.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
mole_runtime.exe --check-wup
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Check WUP Adapter.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
mole_runtime.exe --monitor-wup
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Monitor WUP Input.cmd") -Encoding ASCII

@'
# Mole Game Friend Playtest

This folder is a minimal Windows playtest package for the native Rust SDL3/WUP
runtime. It does not include the full source repository.

## Quick Start

1. If using a Wii U / Switch GameCube adapter, make sure it is available through
   WinUSB/libusb. Zadig's WinUSB driver setup is the usual path.
2. Plug in the adapter and controller before launching.
3. Double-click `Run Mole Game.cmd`.

## Useful Launchers

- `Run Mole Game.cmd`: normal playtest with UCF enabled.
- `Run Mole Game Vanilla No UCF.cmd`: same runtime with UCF disabled so the
  native pre-UCF GameCube input path can be checked.
- `Check WUP Adapter.cmd`: prints whether the adapter ports are visible.
- `Monitor WUP Input.cmd`: opens the native input visualizer.

## Notes

- The game writes input trace logs to `logs\` next to this package.
- The package is intentionally small: executable, SDL3 runtime DLL, background,
  Dolphin Mole sprites, and launch scripts.
- This build is generated from the current `master` Rust runtime.
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "README.md") -Encoding ASCII

Compress-Archive -Path (Join-Path $packageRoot "*") -DestinationPath $zipPath -Force

& rustc (Join-Path $repoRoot "tools\friend_playtest_bootstrap.rs") -O -o $bootstrapExe
Copy-Item -LiteralPath $bootstrapExe -Destination $playtestExe -Force

Write-Host "Created package folder: $packageRoot"
Write-Host "Created package zip:    $zipPath"
Write-Host "Created one-file exe:   $bootstrapExe"
Write-Host "Updated playtest exe:   $playtestExe"
