#!/bin/bash
# Build static Linux binaries using musl
# Requirements:
#   - Rust toolchain
#   - cross (cargo install cross)

set -e

echo "Building yed for Linux (musl static binary)..."

# Check Rust version (cross 0.2.5+ needs rustc 1.92+)
RUSTC_VERSION=$(rustc -V | cut -d' ' -f2)
echo "Rust version: $RUSTC_VERSION"
echo "If cross install fails, run: rustup update"
echo ""

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo "Installing cross..."
    cargo install cross || cargo install cross --version 0.2.4
fi

# Build for x86_64
echo ""
echo "=== Building x86_64-unknown-linux-musl ==="
cross build --release --target x86_64-unknown-linux-musl

# Build for aarch64 (Raspberry Pi 4/5, ARM servers)
echo ""
echo "=== Building aarch64-unknown-linux-musl ==="
cross build --release --target aarch64-unknown-linux-musl

# Build for armv7 (Raspberry Pi 3, 32-bit ARM)
echo ""
echo "=== Building armv7-unknown-linux-musleabihf ==="
cross build --release --target armv7-unknown-linux-musleabihf

echo ""
echo "Build complete!"
echo ""
echo "Binaries:"
echo "  - target/x86_64-unknown-linux-musl/release/yed"
echo "  - target/aarch64-unknown-linux-musl/release/yed"
echo "  - target/armv7-unknown-linux-musleabihf/release/yed"
echo ""
echo "To verify static linking:"
echo "  file target/x86_64-unknown-linux-musl/release/yed"
echo "  ldd target/x86_64-unknown-linux-musl/release/yed"
