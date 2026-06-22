param(
  [string]$AppDir = "dist/windows/app",
  [string]$BuildRoot = "target/release/build"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$appPath = if ([System.IO.Path]::IsPathRooted($AppDir)) {
  $AppDir
} else {
  Join-Path $repoRoot $AppDir
}
$buildRootPath = if ([System.IO.Path]::IsPathRooted($BuildRoot)) {
  $BuildRoot
} else {
  Join-Path $repoRoot $BuildRoot
}

if (-not (Test-Path $appPath)) {
  throw "App staging directory not found: $appPath"
}

$dll = Get-ChildItem -Path $buildRootPath -Recurse -Filter codex_flameshot_embedded.dll |
  Where-Object { $_.FullName -like "*flameshot-embedded-bin*" } |
  Select-Object -First 1
if (-not $dll) {
  throw "Embedded Flameshot DLL not found under $buildRootPath"
}

Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $appPath "codex_flameshot_embedded.dll") -Force

$windeployqt = $env:WINDEPLOYQT
if (-not $windeployqt) {
  $candidate = Get-Command windeployqt.exe -ErrorAction SilentlyContinue
  if ($candidate) {
    $windeployqt = $candidate.Source
  }
}
if (-not $windeployqt -or -not (Test-Path $windeployqt)) {
  throw "windeployqt.exe not found; install Qt or set WINDEPLOYQT"
}

& $windeployqt --release --dir $appPath (Join-Path $appPath "codex-plus-plus.exe")
& $windeployqt --release --dir $appPath (Join-Path $appPath "codex_flameshot_embedded.dll")

$required = @(
  "codex_flameshot_embedded.dll",
  "Qt6Core.dll",
  "Qt6Gui.dll",
  "Qt6Widgets.dll",
  "Qt6Network.dll",
  "Qt6Svg.dll",
  "platforms\qwindows.dll"
)
foreach ($relative in $required) {
  $path = Join-Path $appPath $relative
  if (-not (Test-Path $path)) {
    throw "Missing embedded Flameshot runtime dependency: $path"
  }
}

Write-Host "Embedded Flameshot runtime staged at $appPath"
