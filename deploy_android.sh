#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
# Name of the Rust library target, as defined in Cargo.toml.
LIB_NAME="rtp_midi"

# The Application ID of your Android app.
# This is needed to push the library to the correct app directory.
# Replace with your actual package name when you have an Android app wrapper.
# For now, we push to a temporary directory.
# APP_PACKAGE="com.example.rtpmidi" # Placeholder

# The ABI of the device/emulator to deploy to.
# This script will try to detect it automatically.
TARGET_ABI=""

# --- Check for adb ---
if ! command -v adb &> /dev/null
then
    echo "ERROR: adb could not be found. Please install the Android SDK Platform-Tools and ensure it's in your PATH."
    exit 1
fi

# --- Check for connected device ---
echo "--- Checking for connected devices ---"
# The 'grep -w "device"' ensures we don't match "unauthorized" or other states.
NUM_DEVICES=$(adb devices | grep -c -w "device")

if [ "$NUM_DEVICES" -eq 0 ]; then
    echo "ERROR: No device found. Please connect an Android device or start an emulator."
    exit 1
elif [ "$NUM_DEVICES" -gt 1 ]; then
    echo "WARNING: Multiple devices found. Using the first one available."
    echo "To target a specific device, use 'adb -s <device_id> push ...'"
fi

# --- Detect Device ABI ---
echo "--- Detecting device ABI ---"
# Get the primary ABI of the connected device
DETECTED_ABI=$(adb shell getprop ro.product.cpu.abi)
echo "Detected device ABI: ${DETECTED_ABI}"

# Map the detected ABI to the directory names used in the build script
case ${DETECTED_ABI} in
    "arm64-v8a")
        TARGET_ABI="arm64-v8a"
        ;;
    "armeabi-v7a")
        TARGET_ABI="armeabi-v7a"
        ;;
    "x86_64")
        TARGET_ABI="x86_64"
        ;;
    "x86")
        TARGET_ABI="x86"
        ;;
    *)
        echo "ERROR: Unsupported ABI: ${DETECTED_ABI}. Cannot deploy."
        exit 1
        ;;
esac

# --- Deploy ---
SOURCE_LIB_PATH="target/android_libs/${TARGET_ABI}/lib${LIB_NAME}.so"

if [ ! -f "${SOURCE_LIB_PATH}" ]; then
    echo "ERROR: Build artifact not found at ${SOURCE_LIB_PATH}"
    echo "Please run ./build_android.sh first."
    exit 1
fi

# Pushing to /data/local/tmp is a common strategy for testing native libraries
# without needing a full APK install or root access.
DEST_PATH="/data/local/tmp/lib${LIB_NAME}.so"

echo "--- Deploying library to device ---"
echo "Source:      ${SOURCE_LIB_PATH}"
echo "Destination: ${DEST_PATH}"

adb push "${SOURCE_LIB_PATH}" "${DEST_PATH}"

echo ""
echo "--- Deployment successful ---"
echo "Library pushed to ${DEST_PATH} on the device."
echo "You can access it from the device shell. If needed, set execute permissions: 'chmod 755 ${DEST_PATH}'"

