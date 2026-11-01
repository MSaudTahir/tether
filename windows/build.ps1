# Builds the Rust core, generates its C# bindings, builds the app and puts it in build\Tether-<arch>.
# Usage: .\build.ps1 [-Arch arm64|x64] [-Version 1.0.0]   (default: this PC's architecture)
param(
    [string]$Arch = $(if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }),
    [string]$Version = ""
)
$ErrorActionPreference = "Stop"
# Work in this folder, and return to the caller's afterwards (Set-Location would outlive the script).
Push-Location $PSScriptRoot
try {

    $target = @{ arm64 = "aarch64-pc-windows-msvc"; x64 = "x86_64-pc-windows-msvc" }[$Arch]
    rustup target add $target | Out-Null
    Push-Location ..\core
    cargo build --release --lib --target $target
    if ($LASTEXITCODE) { throw "cargo build failed" }
    # The bindings are the same on every platform, so they can also be generated on another machine (see README).
    if (Get-Command uniffi-bindgen-cs -ErrorAction SilentlyContinue) {
        uniffi-bindgen-cs --library target\$target\release\tether_core.dll --out-dir ..\windows\Generated
    }
    Pop-Location
    if (-not (Test-Path Generated\tether_core.cs)) { throw "Generated\tether_core.cs is missing: install uniffi-bindgen-cs (see README)" }

    New-Item -ItemType Directory -Force .rust\win-$Arch | Out-Null
    Copy-Item ..\core\target\$target\release\tether_core.dll .rust\win-$Arch\

    $versionArg = if ($Version) { "-p:Version=$Version" } else { "-p:Version=0.0.0-dev" }
    dotnet publish -c Release -r win-$Arch -p:Platform=$Arch $versionArg -o build\Tether-$Arch
    if ($LASTEXITCODE) { throw "dotnet publish failed" }
    Write-Host "Built windows\build\Tether-$Arch\Tether.exe"
}
finally {
    Pop-Location
}
