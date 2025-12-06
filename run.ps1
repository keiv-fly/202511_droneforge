Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location -LiteralPath $root

function Ensure-Command {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing command '$Name'. Install with: $InstallHint"
    }
}

Ensure-Command -Name 'cargo' -InstallHint 'Install Rust from https://rustup.rs/ to get cargo.'
Ensure-Command -Name 'rustup' -InstallHint 'Install Rust from https://rustup.rs/ to get rustup.'
Ensure-Command -Name 'simple-http-server' -InstallHint 'cargo install simple-http-server'

$wasm_target = 'wasm32-unknown-unknown'
if (-not (rustup target list --installed | Select-String -SimpleMatch $wasm_target)) {
    Write-Host "Adding target $wasm_target..."
    rustup target add $wasm_target
}

Write-Host "Building droneforge-web for WASM (release)..."
cargo build -p droneforge-web --release --target $wasm_target

$wasm_source = Join-Path $root "target/wasm32-unknown-unknown/release/droneforge-web.wasm"
$web_dir = Join-Path $root "web"
$wasm_dest = Join-Path $web_dir "droneforge-web.wasm"

if (-not (Test-Path $wasm_source)) {
    throw "Build output not found at $wasm_source"
}

New-Item -ItemType Directory -Force -Path $web_dir | Out-Null
Copy-Item -Force -Path $wasm_source -Destination $wasm_dest

$port = 8000
$baseUrl = "http://127.0.0.1:$port/"
$indexUrl = "$baseUrl`index.html"

Write-Host "Starting static file server from '$web_dir' on $baseUrl ..."
$server = Start-Process -FilePath 'simple-http-server' -ArgumentList @('.', '-p', $port) -WorkingDirectory $web_dir -PassThru

Start-Sleep -Seconds 1
Write-Host "Opening browser at $indexUrl"
Start-Process $indexUrl | Out-Null

Write-Host "Server PID: $($server.Id). Stop it later with: Stop-Process -Id $($server.Id)"

