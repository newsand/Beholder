#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    $here = $PSScriptRoot
    if (Test-Path (Join-Path $here "Cargo.toml")) {
        return $here
    }
    $parent = Split-Path -Parent $here
    if (Test-Path (Join-Path $parent "Cargo.toml")) {
        return $parent
    }
    throw "Cargo.toml not found. Run this script from the Beholder repo."
}

if ($env:OS -ne "Windows_NT") {
    throw "This script must run on Windows with the MSVC toolchain."
}

$Root = Get-RepoRoot
Set-Location $Root

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust from https://rustup.rs and the Visual Studio C++ workload."
}

$cargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($cargoToml -notmatch 'version = "([^"]+)"') {
    throw "Could not read version from Cargo.toml"
}
$version = $Matches[1]

Write-Host "==> cargo test --workspace"
cargo test --workspace
if ($LASTEXITCODE -ne 0) { throw "tests failed" }

Write-Host "==> cargo build --release --workspace"
cargo build --release --workspace
if ($LASTEXITCODE -ne 0) { throw "release build failed" }

$exe = Join-Path $Root "target\release\beholder.exe"
if (-not (Test-Path $exe)) {
    throw "beholder.exe not found at $exe"
}

$outDir = Join-Path $Root "dist\windows"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

Get-ChildItem $outDir -ErrorAction SilentlyContinue | Remove-Item -Force

$named = "beholder-$version-windows-x86_64.exe"
Copy-Item $exe (Join-Path $outDir $named) -Force
Copy-Item $exe (Join-Path $outDir "beholder.exe") -Force

$zipPath = Join-Path $outDir "beholder-$version-windows-x86_64.zip"
Compress-Archive -Path (Join-Path $outDir $named) -DestinationPath $zipPath -Force

Write-Host ""
Write-Host "Release written to $outDir"
Get-ChildItem $outDir | Format-Table Name, @{Label = "SizeMB"; Expression = { "{0:N2}" -f ($_.Length / 1MB) } } -AutoSize
