$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$distRoot = Join-Path $repoRoot "dist"
$playtestRoot = Join-Path $repoRoot "playtest"
$packageName = "MoleGame-FriendPlaytest"
$localInternetPackageName = "MoleGame-LocalInternetPlaytest"
$packageRoot = Join-Path $distRoot $packageName
$zipPath = Join-Path $distRoot "$packageName.zip"
$bootstrapExe = Join-Path $distRoot "$packageName.exe"
$localInternetBootstrapExe = Join-Path $distRoot "$localInternetPackageName.exe"
$playtestExe = Join-Path $playtestRoot "$packageName.exe"
$localInternetPlaytestExe = Join-Path $playtestRoot "$localInternetPackageName.exe"

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
Remove-PathInsideDist -Path $localInternetBootstrapExe
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
mole_runtime.exe --friend-connect --play --input-trace --netplay-delay 2
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Mole Game.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
echo Running Friend Connect with vanilla pre-UCF input mode.
mole_runtime.exe --friend-connect --play --input-trace --no-ucf --netplay-delay 2
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Mole Game Vanilla No UCF.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
echo Starting visible Friend Connect host for solo internet testing.
echo Copy the YOUR CODE value from the Friend Connect window, then run:
echo   Run Headless Internet Peer.cmd
mole_runtime.exe --friend-connect --play --input-trace --netplay-delay 2
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Solo Internet Host.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
set /p HOST_CODE=Enter visible host Friend Connect code:
if "%HOST_CODE%"=="" (
    echo Host code is required.
    pause
    exit /b 1
)
mole_runtime.exe --friend-connect-headless-peer --connect-code %HOST_CODE% --netplay-delay 2
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Headless Internet Peer.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
for /f %%I in ('powershell -NoProfile -ExecutionPolicy Bypass -Command "$n=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -bxor $PID; 'L{0:X5}' -f ($n -band 0xFFFFF)"') do set "HOST_CODE=%%I"
for /f %%I in ('powershell -NoProfile -ExecutionPolicy Bypass -Command "$n=([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + 7919) -bxor ($PID -shl 1); 'P{0:X5}' -f ($n -band 0xFFFFF)"') do set "PEER_CODE=%%I"
if "%HOST_CODE%"=="" set "HOST_CODE=LOCAL1"
if "%PEER_CODE%"=="" set "PEER_CODE=PEER01"
echo Starting local internet playtest with code %HOST_CODE%.
echo Starting visible P1 host and visible P2 peer on loopback UDP ports 41001/41002.
echo Supabase is still used for setup; local UDP is used for same-machine gameplay packets.
echo Logs will write under logs\netplay.
echo CPU headroom is shown as CPU nHZ in each Friend Connect status panel.
start "Mole Solo Host" "%~dp0mole_runtime.exe" --friend-connect --play --input-trace --netplay-delay 2 --friend-code %HOST_CODE% --auto-start --window-offset-x 0 --window-offset-y 0 --friend-local-udp 127.0.0.1:41001
timeout /t 2 /nobreak >nul
start "Mole Solo Visual Peer" "%~dp0mole_runtime.exe" --friend-connect --play --input-trace --netplay-delay 2 --friend-code %PEER_CODE% --connect-code %HOST_CODE% --window-offset-x 360 --window-offset-y 400 --friend-local-udp 127.0.0.1:41002
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Local Internet Playtest.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
mole_runtime.exe --sdl --play --input-trace
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Local Practice.cmd") -Encoding ASCII

@'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0;%PATH%"
set "MOLE_ASSET_ROOT=%~dp0"
echo Running local practice with vanilla pre-UCF input mode.
mole_runtime.exe --sdl --play --input-trace --no-ucf
pause
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "Run Local Practice Vanilla No UCF.cmd") -Encoding ASCII

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

This folder is a self-contained Windows playtest package for the native Rust
SDL3/WUP runtime. It does not include the full source repository.

## Quick Start

1. If using a Wii U / Switch GameCube adapter, make sure it is available through
   WinUSB/libusb. Zadig's WinUSB driver setup is the usual path.
2. Plug in the adapter and controller before launching if you have one
   available. A missing or busy adapter is nonfatal; the game keeps waiting and
   retrying.
3. Double-click `Run Mole Game.cmd`.
4. One player shares the code shown in their Friend Connect window.
5. The other player types that code and presses Enter.
6. When the peer appears in the lobby, the code-owner clicks Start.

## Useful Launchers

- `Run Mole Game.cmd`: Friend Connect playtest with UCF enabled. It opens the
  game window and a second connection-code window, then waits until closed. It
  uses the Slippi-style default `--netplay-delay 2` baseline.
- `Run Mole Game Vanilla No UCF.cmd`: Friend Connect with UCF disabled so the
  native pre-UCF GameCube input path can be checked. It uses the same
  `--netplay-delay 2` baseline.
- `Run Solo Internet Host.cmd`: starts the normal visible P1 host for a solo
  internet-path test on one machine.
- `Run Headless Internet Peer.cmd`: asks for the visible host code, then starts
  a headless P2 joiner that uses Supabase setup, direct UDP gameplay packets,
  the same delay/repair buffer, and deterministic neutral P2 input.
- `Run Local Internet Playtest.cmd`: generates a local Friend Connect code and
  starts both a visible P1 host and visible P2 peer automatically. The host
  auto-starts when the peer connects. Supabase still handles setup; same-machine
  gameplay packets use explicit loopback UDP ports so router NAT hairpinning
  cannot hide packet-flow failures.
- `Run Local Practice.cmd`: local single-machine playtest with UCF enabled.
- `Run Local Practice Vanilla No UCF.cmd`: local single-machine vanilla input
  playtest.
- `Check WUP Adapter.cmd`: prints whether the adapter ports are visible.
- `Monitor WUP Input.cmd`: opens the native input visualizer.

## Notes

- The game writes input trace logs to `logs\` next to this package.
- Supabase is used only to exchange setup endpoints through the public
  publishable project key. Gameplay packets are direct UDP between players.
- Start is rebroadcast briefly as setup signaling so the joining player does
  not miss it while their listener finishes connecting.
- The code-owner is always P1 and the only player who can click Start. The
  player who types the shared code is always P2 and waits for Start.
- Controllers are inactive at launch. The first connected local WUP/GameCube
  controller that produces non-neutral gameplay input latches as that machine's
  active controller. The code-owner's active controller drives rollback P1; the
  joiner's active controller drives rollback P2. Extra local controllers are
  ignored for this singles playtest and reserved for future local doubles.
- Gameplay packet frames start at match frame `0`, not at each machine's window
  launch frame. Local input is committed through a default 2-frame netplay
  delay, and the newest 8 committed input frames are retransmitted each frame
  until ACKs allow old packets to drop.
- Solo internet testing writes compact JSONL role logs under `logs\netplay`.
  Logs summarize setup and every 60 gameplay frames, record anomalies
  immediately, and cap at 5 MB per role/session instead of dumping every packet.
- The package includes the compiled runtime, SDL3 runtime DLL, generated
  source-frame data embedded in the runtime, background, Dolphin Mole sprites,
  and launch scripts.
- This build is generated from the current `master` Rust runtime.
- `MoleGame-LocalInternetPlaytest.exe` is a secondary one-file bootstrap that
  extracts this same package and runs `Run Local Internet Playtest.cmd`.
'@ | Set-Content -LiteralPath (Join-Path $packageRoot "README.md") -Encoding ASCII

Compress-Archive -Path (Join-Path $packageRoot "*") -DestinationPath $zipPath -Force

& rustc (Join-Path $repoRoot "tools\friend_playtest_bootstrap.rs") -O -o $bootstrapExe
& rustc (Join-Path $repoRoot "tools\local_internet_playtest_bootstrap.rs") -O -o $localInternetBootstrapExe
Copy-Item -LiteralPath $bootstrapExe -Destination $playtestExe -Force
Copy-Item -LiteralPath $localInternetBootstrapExe -Destination $localInternetPlaytestExe -Force

Write-Host "Created package folder: $packageRoot"
Write-Host "Created package zip:    $zipPath"
Write-Host "Created one-file exe:   $bootstrapExe"
Write-Host "Created local internet one-file exe: $localInternetBootstrapExe"
Write-Host "Updated playtest exe:   $playtestExe"
Write-Host "Updated local internet playtest exe: $localInternetPlaytestExe"
