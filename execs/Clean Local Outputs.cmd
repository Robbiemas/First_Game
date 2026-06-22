@echo off
setlocal

cd /d "%~dp0.."

set "MODE=%~1"

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference = 'Stop';" ^
  "$root = [System.IO.Path]::GetFullPath((Get-Location).Path);" ^
  "$rootFull = $root.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar;" ^
  "$apply = ('%MODE%' -eq '--apply');" ^
  "$candidates = @('debug', 'logs', '.pytest_cache', '__pycache__');" ^
  "$execLogDir = Join-Path $root 'execs';" ^
  "if (Test-Path -LiteralPath $execLogDir) { $candidates += Get-ChildItem -LiteralPath $execLogDir -Force -File -Filter '*.log' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }; }" ^
  "$paths = foreach ($candidate in $candidates) { $full = if ([System.IO.Path]::IsPathRooted($candidate)) { [System.IO.Path]::GetFullPath($candidate) } else { [System.IO.Path]::GetFullPath((Join-Path $root $candidate)) }; if (Test-Path -LiteralPath $full) { if (-not $full.StartsWith($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) { throw \"Refusing to clean outside workspace: $full\" }; $full } };" ^
  "$paths = @($paths | Sort-Object -Unique);" ^
  "if ($paths.Count -eq 0) { Write-Host 'No local outputs found.'; exit 0 }" ^
  "if (-not $apply) { Write-Host 'Dry run. Pass --apply to remove these ignored local outputs:'; $paths | ForEach-Object { Write-Host ('  ' + $_) }; exit 0 }" ^
  "$paths | ForEach-Object { Write-Host ('Removing ' + $_); Remove-Item -LiteralPath $_ -Recurse -Force -ErrorAction Stop }"

exit /b %ERRORLEVEL%
