#!/bin/bash
set -e

if [[ "$1" == "--help" ]]; then
  echo "Usage: $0 [--help]"
  echo "Packages release artifacts for all platforms into dist/<platform>/"
  exit 0
fi

DIST=dist
VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
mkdir -p $DIST

# PC Linux
LINUX_DIST=$DIST/linux
mkdir -p $LINUX_DIST
cp target/release/rtp_midi_node $LINUX_DIST/ || { echo "rtp_midi_node missing"; exit 1; }
cp config.toml $LINUX_DIST/ || cp config.toml.example $LINUX_DIST/config.toml || true
cp -r frontend $LINUX_DIST/
cp -r ui-frontend $LINUX_DIST/ || true
cp README.md $LINUX_DIST/ || true
cp LICENSE* $LINUX_DIST/ || true
(cd $DIST && zip -r linux_release_$VERSION.zip linux)
sha256sum $DIST/linux_release_$VERSION.zip > $DIST/linux_release_$VERSION.zip.sha256

echo "Packaged Linux release in $LINUX_DIST and $DIST/linux_release_$VERSION.zip"

# Android (shared libs)
ANDROID_DIST=$DIST/android
mkdir -p $ANDROID_DIST
cp -r target/android_libs $ANDROID_DIST/ || true
cp config.toml $ANDROID_DIST/ || cp config.toml.example $ANDROID_DIST/config.toml || true
cp README.md $ANDROID_DIST/ || true
cp LICENSE* $ANDROID_DIST/ || true
(cd $DIST && zip -r android_release_$VERSION.zip android)
sha256sum $DIST/android_release_$VERSION.zip > $DIST/android_release_$VERSION.zip.sha256

echo "Packaged Android release in $ANDROID_DIST and $DIST/android_release_$VERSION.zip"

# ESP32
ESP32_DIST=$DIST/esp32
mkdir -p $ESP32_DIST
cp target/xtensa-esp32-none-elf/release/* $ESP32_DIST/ || true
cp config.toml $ESP32_DIST/ || cp config.toml.example $ESP32_DIST/config.toml || true
cp README.md $ESP32_DIST/ || true
cp LICENSE* $ESP32_DIST/ || true
(cd $DIST && zip -r esp32_release_$VERSION.zip esp32)
sha256sum $DIST/esp32_release_$VERSION.zip > $DIST/esp32_release_$VERSION.zip.sha256

echo "Packaged ESP32 release in $ESP32_DIST and $DIST/esp32_release_$VERSION.zip"

echo "--- All platform releases packaged in $DIST/ ---" 