#!/bin/bash
set -e

# Build all binaries and libraries for PC Linux (x86_64)
# Uses all default features unless overridden

# Optional: set features (edit as needed)
FEATURES="hal_pc,ui"

# Build workspace
cargo build --release --features "$FEATURES"

# List built artifacts
ls -lh target/release/

echo "--- PC Linux build finished successfully ---" 