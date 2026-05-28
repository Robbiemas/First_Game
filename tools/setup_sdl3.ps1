$ErrorActionPreference = "Stop"

$version = "3.4.8"
$repoRoot = Split-Path -Parent $PSScriptRoot
$localRoot = Join-Path $repoRoot ".local"
$sdlRoot = Join-Path $localRoot "SDL3"
$archive = Join-Path $localRoot "SDL3-devel-$version-VC.zip"
$url = "https://github.com/libsdl-org/SDL/releases/download/release-$version/SDL3-devel-$version-VC.zip"

New-Item -ItemType Directory -Force -Path $localRoot | Out-Null

if (-not (Test-Path -LiteralPath (Join-Path $sdlRoot "lib\x64\SDL3.lib"))) {
    Write-Host "Downloading SDL3 $version..."
    Invoke-WebRequest -Uri $url -OutFile $archive

    $extractRoot = Join-Path $localRoot "SDL3-extract"
    if (Test-Path -LiteralPath $extractRoot) {
        Remove-Item -LiteralPath $extractRoot -Recurse -Force
    }
    Expand-Archive -LiteralPath $archive -DestinationPath $extractRoot -Force

    $packageRoot = Get-ChildItem -LiteralPath $extractRoot -Directory |
        Where-Object { $_.Name -like "SDL3-$version*" } |
        Select-Object -First 1

    if ($null -eq $packageRoot) {
        throw "Could not find SDL3 package root after extraction."
    }

    if (Test-Path -LiteralPath $sdlRoot) {
        Remove-Item -LiteralPath $sdlRoot -Recurse -Force
    }
    Move-Item -LiteralPath $packageRoot.FullName -Destination $sdlRoot
    Remove-Item -LiteralPath $extractRoot -Recurse -Force
}

$libPath = Join-Path $sdlRoot "lib\x64"
$dllPath = Join-Path $sdlRoot "lib\x64"

if (-not (Test-Path -LiteralPath (Join-Path $libPath "SDL3.lib"))) {
    throw "SDL3.lib was not found at $libPath."
}

if (-not (Test-Path -LiteralPath (Join-Path $dllPath "SDL3.dll"))) {
    throw "SDL3.dll was not found at $dllPath."
}

Write-Host "SDL3 ready:"
Write-Host "  $libPath"
