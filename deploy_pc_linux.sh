#!/bin/bash
set -e

# Deploy all built binaries and libraries to a target directory
# Usage: ./deploy_pc_linux.sh /path/to/deploy

DEPLOY_DIR=${1:-$HOME/rtp-midi-deploy}

mkdir -p "$DEPLOY_DIR"
cp -v target/release/rtp-midi* "$DEPLOY_DIR/"
cp -v target/release/*.so "$DEPLOY_DIR/" || true

ls -lh "$DEPLOY_DIR"
echo "--- PC Linux deploy finished successfully ---" 