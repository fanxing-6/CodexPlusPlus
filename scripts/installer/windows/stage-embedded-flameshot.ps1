param(
  [string]$AppDir = "dist/windows/app",
  [string]$BuildRoot = "target/release/build"
)

$ErrorActionPreference = "Stop"

function Get-PeDependentDlls {
  param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath
  )

  $dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
  if ($dumpbin) {
    $output = & $dumpbin.Source /DEPENDENTS $BinaryPath 2>&1
  } else {
    $link = Get-Command link.exe -ErrorAction SilentlyContinue
    if (-not $link) {
      throw "dumpbin.exe or link.exe is required to verify Windows runtime dependencies"
    }
    $output = & $link.Source /DUMP /DEPENDENTS $BinaryPath 2>&1
  }

  if ($LASTEXITCODE -ne 0) {
    throw "Failed to inspect PE dependencies for ${BinaryPath}: $($output -join [Environment]::NewLine)"
  }

  $dlls = @()
  foreach ($line in $output) {
    if ($line -match '^\s*([A-Za-z0-9_.+-]+\.dll)\s*$') {
      $dlls += $Matches[1]
    }
  }
  return $dlls | Sort-Object -Unique
}

function Assert-NoDebugQtDependencies {
  param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [Parameter(Mandatory = $true)]
    [string]$AppPath
  )

  $dlls = Get-PeDependentDlls -BinaryPath $BinaryPath
  $debugQtDlls = $dlls | Where-Object { $_ -match '^Qt6[A-Za-z0-9_]*d\.dll$' }
  if ($debugQtDlls) {
    throw "Release package binary depends on debug Qt DLL(s): $([string]::Join(', ', $debugQtDlls)) in $BinaryPath"
  }

  $qtDlls = $dlls | Where-Object { $_ -match '^Qt6[A-Za-z0-9_]*\.dll$' }
  foreach ($qtDll in $qtDlls) {
    $qtPath = Join-Path $AppPath $qtDll
    if (-not (Test-Path $qtPath)) {
      throw "Qt runtime dependency imported by $BinaryPath was not staged: $qtDll"
    }
  }
}

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

Assert-NoDebugQtDependencies `
  -BinaryPath (Join-Path $appPath "codex-plus-plus.exe") `
  -AppPath $appPath
Assert-NoDebugQtDependencies `
  -BinaryPath (Join-Path $appPath "codex_flameshot_embedded.dll") `
  -AppPath $appPath

Write-Host "Embedded Flameshot runtime staged at $appPath"
