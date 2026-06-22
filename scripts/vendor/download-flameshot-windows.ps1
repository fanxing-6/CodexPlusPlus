param(
  [string]$Destination = "dist/windows/app/tools/flameshot",
  [string]$Version = "14.0.0"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$destinationPath = if ([System.IO.Path]::IsPathRooted($Destination)) {
  $Destination
} else {
  Join-Path $repoRoot $Destination
}

$assetName = "flameshot-$Version-win64.zip"
$url = "https://github.com/flameshot-org/flameshot/releases/download/v$Version/$assetName"
$workDir = Join-Path ([System.IO.Path]::GetTempPath()) "codex-plus-flameshot-$([System.Guid]::NewGuid().ToString('N'))"
$zipPath = Join-Path $workDir $assetName
$extractDir = Join-Path $workDir "extract"

New-Item -ItemType Directory -Force $workDir, $extractDir | Out-Null
try {
  Invoke-WebRequest -Uri $url -OutFile $zipPath
  Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

  $root = Get-ChildItem -Path $extractDir -Directory | Select-Object -First 1
  if (-not $root) {
    throw "Flameshot archive did not contain a root directory"
  }
  $binDir = Join-Path $root.FullName "bin"
  if (-not (Test-Path (Join-Path $binDir "flameshot-cli.exe"))) {
    throw "Flameshot archive did not contain bin\flameshot-cli.exe"
  }

  Remove-Item -LiteralPath $destinationPath -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $destinationPath | Out-Null
  Copy-Item -Path (Join-Path $binDir "*") -Destination $destinationPath -Recurse -Force

  if (-not (Test-Path (Join-Path $destinationPath "flameshot-cli.exe"))) {
    throw "Failed to stage bundled Flameshot"
  }
  Write-Host "Bundled Flameshot staged at $destinationPath"
} finally {
  Remove-Item -LiteralPath $workDir -Recurse -Force -ErrorAction SilentlyContinue
}
