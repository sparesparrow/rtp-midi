#!/bin/bash
set -e

# Build for ESP32 (WROOM)
# Requires espup, esp-idf, and Rust ESP32 toolchain
# See: https://github.com/esp-rs/esp-idf-template

# Set target triple for ESP32
TARGET="xtensa-esp32-none-elf"

# Optional: set features (edit as needed)
FEATURES="hal_esp32"

# Check for espup/cargo-espflash
if ! command -v cargo-espflash &> /dev/null; then
    echo "ERROR: cargo-espflash not found. Install with: cargo install cargo-espflash"
    exit 1
fi

# Build
cargo build --release --target $TARGET --features "$FEATURES"

# List built artifacts
ls -lh target/$TARGET/release/

echo "--- ESP32 build finished successfully ---" 