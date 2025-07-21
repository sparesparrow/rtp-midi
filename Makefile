# Makefile for rtp-midi project dependency hygiene and update automation

.PHONY: check-rust update-rust check-android update-android check-esp32 update-esp32 all-checks all-updates

check-rust:
	cargo outdated || echo "Install cargo-outdated with: cargo install cargo-outdated"
	cargo audit || echo "Install cargo-audit with: cargo install cargo-audit"

update-rust:
	cargo update

check-android:
	cd android_hub && ./gradlew dependencyUpdates || echo "Apply the Gradle Versions Plugin if not present."

update-android:
	cd android_hub && ./gradlew build --refresh-dependencies

check-esp32:
	cd firmware/esp32_visualizer && pio update || echo "Install PlatformIO with: pip install platformio"

update-esp32:
	cd firmware/esp32_visualizer && pio update && pio run

all-checks: check-rust check-android check-esp32

all-updates: update-rust update-android update-esp32 