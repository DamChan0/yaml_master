# Build script for Windows
# Builds release binary for Windows (x86_64)

param(
    [switch]$LinuxMusl  # Optional: build Linux x86_64 musl (requires Docker + cross)
)

if ($LinuxMusl) {
    Write-Host "Building yaml_master for Linux x86_64 (musl)..." -ForegroundColor Cyan
    Write-Host "Requires: Docker Desktop running, cross installed (cargo install cross)" -ForegroundColor Yellow
    cross build --release --target x86_64-unknown-linux-musl
    Write-Host ""
    Write-Host "Binary: target\x86_64-unknown-linux-musl\release\yaml_master" -ForegroundColor Green
} else {
    Write-Host "Building yaml_master for Windows (x86_64)..." -ForegroundColor Cyan
    cargo build --release
    Write-Host ""
    Write-Host "Build complete!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Binary location:"
    Write-Host "  - target\release\yaml_master.exe"
    Write-Host ""
    Write-Host "To install system-wide, copy to a directory in your PATH:"
    Write-Host "  copy target\release\yaml_master.exe C:\Users\$env:USERNAME\.cargo\bin\"
    Write-Host ""
    Write-Host "To build Linux x86_64 musl (requires Docker):"
    Write-Host "  .\scripts\build-all.ps1 -LinuxMusl"
}
