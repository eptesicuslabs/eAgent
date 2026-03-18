$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$distRoot = Join-Path $repoRoot "dist"
$portableRoot = Join-Path $distRoot "eCode-portable"
$zipPath = Join-Path $distRoot "eCode-portable-windows-x64.zip"

New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
Remove-Item -Recurse -Force $portableRoot -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null

$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release --target x86_64-pc-windows-msvc

$exeSource = Join-Path $repoRoot "target\x86_64-pc-windows-msvc\release\ecode.exe"
$exeDest = Join-Path $portableRoot "eCode.exe"
Copy-Item $exeSource $exeDest -Force

$readme = @"
eCode Portable

- Run `eCode.exe`.
- The application stores config, logs, attachments, and the event store in `eCode-data\` next to the executable.
- No installer is required.
- `llama-server` and local GGUF models are not bundled; configure their paths in Settings if you want local inference.
"@

Set-Content -Path (Join-Path $portableRoot "README.txt") -Value $readme -Encoding ASCII

if (Test-Path $zipPath) {
    Remove-Item $zipPath -Force
}

Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath $zipPath
Write-Host "Portable build written to $portableRoot"
Write-Host "Zip written to $zipPath"
