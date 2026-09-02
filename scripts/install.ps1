# Windows install. Run in PowerShell:
#   irm https://www.copypaste.fyi/install.ps1 | iex
$ErrorActionPreference = "Stop"

function Have($name) {
  return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

Write-Host "copypaste installer  os=windows"

if (Have "brew") {
  Write-Host "plan: brew install qxlsz/copypaste/copypaste"
  brew install qxlsz/copypaste/copypaste
} elseif (Have "cargo") {
  Write-Host "plan: cargo install copypaste"
  cargo install copypaste
} elseif (Have "docker") {
  Write-Host "plan: docker compose up --build"
  Write-Host "Clone https://github.com/qxlsz/copypaste.fyi and run docker compose up --build"
} else {
  Write-Host "Install one of: Rust (https://rustup.rs), Docker Desktop, or Git + cargo."
  Write-Host "Then: cargo install copypaste"
  Write-Host "Host internally: `$env:ROCKET_ADDRESS='127.0.0.1'; copypaste serve"
  exit 1
}

Write-Host "Send: copypaste send --host https://www.copypaste.fyi `"notes`""
Write-Host "Host: `$env:ROCKET_ADDRESS='127.0.0.1'; copypaste serve"
