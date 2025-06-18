#!/bin/bash
set -e

# package_release.sh: Collects release artifacts for all platforms into dist/<platform>/
# Usage: Run after building for each platform (see build_*.sh)
# Optionally creates zip archives for easy distribution.

DIST=dist
mkdir -p $DIST

# PC Linux
LINUX_DIST=$DIST/linux
mkdir -p $LINUX_DIST
cp target/release/rtp_midi_node $LINUX_DIST/ || true
cp config.toml $LINUX_DIST/ || true
cp -r frontend $LINUX_DIST/
cp -r ui-frontend $LINUX_DIST/ || true
(cd $DIST && zip -r linux_release.zip linux)

echo "Packaged Linux release in $LINUX_DIST and $DIST/linux_release.zip"

# Android (shared libs)
ANDROID_DIST=$DIST/android
mkdir -p $ANDROID_DIST
cp -r target/android_libs $ANDROID_DIST/
cp config.toml $ANDROID_DIST/ || true
(cd $DIST && zip -r android_release.zip android)

echo "Packaged Android release in $ANDROID_DIST and $DIST/android_release.zip"

# ESP32
ESP32_DIST=$DIST/esp32
mkdir -p $ESP32_DIST
cp target/xtensa-esp32-none-elf/release/* $ESP32_DIST/ || true
cp config.toml $ESP32_DIST/ || true
(cd $DIST && zip -r esp32_release.zip esp32)

echo "Packaged ESP32 release in $ESP32_DIST and $DIST/esp32_release.zip"

echo "--- All platform releases packaged in $DIST/ ---" 