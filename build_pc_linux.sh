#!/bin/bash
set -e

# Build the main binary for PC Linux (x86_64)

cargo build --release --bin rtp_midi_node

# List built artifacts
ls -lh target/release/rtp_midi_node

echo "--- PC Linux build finished successfully ---" 