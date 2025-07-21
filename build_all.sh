#!/bin/bash

# rtp-midi: Comprehensive Build Script
# Builds all binaries and packages for different platforms

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
BUILD_DIR="build"
DIST_DIR="dist"

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    rm -rf "$BUILD_DIR"
}

# Setup directories
setup_dirs() {
    log_info "Setting up build directories..."
    mkdir -p "$BUILD_DIR"
    mkdir -p "$DIST_DIR"
}

# Build Rust binaries
build_rust() {
    log_info "Building Rust binaries..."
    
    # Build release binaries
    cargo build --release --workspace
    
    # Copy binaries to build directory
    cp target/release/rtp_midi_node "$BUILD_DIR/"
    
    log_success "Rust binaries built successfully"
}

# Build Docker image
build_docker() {
    log_info "Building Docker image..."
    
    docker build -t rtp-midi:latest .
    docker tag rtp-midi:latest rtp-midi:$VERSION
    
    log_success "Docker image built successfully"
}

# Build Android (if possible)
build_android() {
    log_info "Building Android components..."
    
    if command -v cargo-ndk &> /dev/null; then
        cd android_hub
        ./gradlew assembleRelease
        cd ..
        log_success "Android build completed"
    else
        log_warning "cargo-ndk not found, skipping Android build"
    fi
}

# Build ESP32 (if possible)
build_esp32() {
    log_info "Building ESP32 components..."
    
    if command -v pio &> /dev/null; then
        cd firmware/esp32_visualizer
        pio run
        cd ../..
        log_success "ESP32 build completed"
    else
        log_warning "PlatformIO not found, skipping ESP32 build"
    fi
}

# Create packages
create_packages() {
    log_info "Creating packages..."
    
    # Create Linux package
    tar -czf "$DIST_DIR/rtp-midi-linux-x86_64-$VERSION.tar.gz" \
        -C "$BUILD_DIR" rtp_midi_node \
        -C .. config.toml README.md
    
    # Create Docker package
    docker save rtp-midi:$VERSION | gzip > "$DIST_DIR/rtp-midi-docker-$VERSION.tar.gz"
    
    log_success "Packages created successfully"
}

# Run tests
run_tests() {
    log_info "Running tests..."
    
    cargo test --workspace
    
    log_success "Tests completed successfully"
}

# Main build process
main() {
    log_info "Starting rtp-midi build process (version $VERSION)"
    
    # Setup
    setup_dirs
    
    # Build components
    build_rust
    build_docker
    build_android
    build_esp32
    
    # Test
    run_tests
    
    # Package
    create_packages
    
    log_success "Build process completed successfully!"
    log_info "Artifacts available in: $DIST_DIR/"
    
    # Show summary
    echo
    log_info "Build Summary:"
    echo "  - Rust binaries: $BUILD_DIR/rtp_midi_node"
    echo "  - Docker image: rtp-midi:$VERSION"
    echo "  - Packages: $DIST_DIR/"
    echo "  - Version: $VERSION"
}

# Handle cleanup on exit
trap cleanup EXIT

# Run main function
main "$@" 