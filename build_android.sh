#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
# Name of the Rust library target, from Cargo.toml [lib] name.
# This will be compiled into librtp_midi.so
LIB_NAME="rtp_midi"

# Android NDK path. Update this if your NDK is in a different location.
# Check for ANDROID_NDK_HOME, otherwise try to find it automatically.
if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "WARNING: ANDROID_NDK_HOME is not set."
    # Try to find it in common locations for macOS and Linux
    if [ -d "$HOME/Library/Android/sdk/ndk-bundle" ]; then
        export ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk-bundle"
        echo "Found NDK at: $ANDROID_NDK_HOME"
    elif [ -d "$HOME/Android/Sdk/ndk" ]; then
        # This path structure is common with Android Studio's SDK manager
        NDK_VERSION=$(ls "$HOME/Android/Sdk/ndk" | sort -r | head -n 1)
        export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/$NDK_VERSION"
        echo "Found NDK at: $ANDROID_NDK_HOME"
    else
        echo "ERROR: Could not find Android NDK. Please set the ANDROID_NDK_HOME environment variable."
        exit 1
    fi
fi

# --- Toolchain and Targets ---
# Add Android targets using rustup
echo "--- Adding Rust Android targets ---"
rustup target add aarch64-linux-android armv7-linux-androideabi

# --- Cargo Configuration for Android ---
# Create a .cargo/config.toml to specify the NDK linkers for each target.
# This is a cleaner approach than setting environment variables.
mkdir -p .cargo
cat > .cargo/config.toml << EOL
# Cargo configuration for Android cross-compilation

[target.aarch64-linux-android]
ar = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang"

[target.armv7-linux-androideabi]
ar = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi21-clang"

# You can add other targets like i686 and x86_64 here if needed
# [target.i686-linux-android]
# ar = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
# linker = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/i686-linux-android21-clang"
#
# [target.x86_64-linux-android]
# ar = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
# linker = "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android21-clang"
EOL

echo "--- Created .cargo/config.toml for NDK toolchains ---"

# --- Build Function ---
# This function builds for a specific target and copies the output.
build_for_target() {
    local TARGET=$1
    local ARCH_ABI=$2
    echo ""
    echo "================================================="
    echo " Building for ${TARGET} (${ARCH_ABI})"
    echo "================================================="

    cargo build --target ${TARGET} --release --lib

    local OUTPUT_DIR="target/android_libs/${ARCH_ABI}"
    mkdir -p "${OUTPUT_DIR}"
    cp "target/${TARGET}/release/lib${LIB_NAME}.so" "${OUTPUT_DIR}/lib${LIB_NAME}.so"
    echo "Successfully built for ${TARGET} and copied to ${OUTPUT_DIR}/"
}

# --- Build for all targets ---
# Build for the most common architectures
build_for_target "aarch64-linux-android" "arm64-v8a"
build_for_target "armv7-linux-androideabi" "armeabi-v7a"

echo ""
echo "--- Android build finished successfully ---"
echo "Shared libraries are located in: target/android_libs/"

