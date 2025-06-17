#!/bin/bash
set -e

# Deploy firmware to ESP32 using cargo-espflash
# Usage: ./deploy_esp32.sh [serial-port]

PORT=${1:-auto}
TARGET="xtensa-esp32-none-elf"
BIN_NAME="rtp_midi_node" # upravte podle názvu binárky

if ! command -v cargo-espflash &> /dev/null; then
    echo "ERROR: cargo-espflash not found. Install with: cargo install cargo-espflash"
    exit 1
fi

if [ "$PORT" = "auto" ]; then
    cargo espflash --release --target $TARGET
else
    cargo espflash --release --target $TARGET --port "$PORT"
fi

echo "--- ESP32 deploy finished successfully ---" 