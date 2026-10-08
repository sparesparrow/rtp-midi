# rtp-midi

*[Česká verze](README.cs.md)*

A modular Rust system for low-latency MIDI routing, audio-reactive visuals and LED control. A hardware
controller (e.g. Native Instruments Maschine) plugs into an Android phone; the phone routes its MIDI to
a DAW over **RTP-MIDI** (AppleMIDI) and to an ESP32 LED visualiser over **OSC**.

```
Maschine ──USB MIDI──▶ Android Hub ──RTP-MIDI──▶ DAW
                            └──────OSC───────▶ ESP32 ──▶ addressable LEDs
```

## Highlights

- **Modular workspace** — separate crates for `core`, `network`, `audio`, `output`, `platform` and
  hardware abstraction layers (`hal-*`).
- **Android Hub** ([`android_hub/`](android_hub)) — Kotlin UI on a Rust NDK core: foreground service,
  AMidi NDK, mDNS discovery, RTP-MIDI to the DAW and OSC to the ESP32.
- **ESP32 visualiser** ([`firmware/esp32_visualizer/`](firmware/esp32_visualizer)) — Arduino core,
  FastLED, dual-core FreeRTOS and an OSC server, with configuration-driven hardware.
- **Protocols** — AppleMIDI handshake and clock sync, DDP receiver, OSC.
- **CI/CD** — tests, lint, `cargo-deny` security audit, releases, and Docker images published to
  [GitHub Container Registry](https://github.com/sparesparrow/rtp-midi/pkgs/container/rtp-midi).

## Quick start (Linux)

Requires stable Rust ([rustup.rs](https://rustup.rs)).

```sh
git clone https://github.com/sparesparrow/rtp-midi.git
cd rtp-midi
# adjust config.toml if needed
cargo run --release --bin rtp_midi_node -- --role server
```

`rtp_midi_node` takes `--role server`, `client` or `ui-host`.

### Docker

```sh
docker run -it --rm -p 5004:5004/udp ghcr.io/sparesparrow/rtp-midi:latest
# with your own configuration
docker run -it --rm -v "$PWD/config.toml:/app/config.toml" -p 5004:5004/udp ghcr.io/sparesparrow/rtp-midi:latest
```

## Platforms

| Platform | Status | Build |
|---|---|---|
| Linux | ✅ Supported | `cargo build --release` |
| Android | ✅ Supported | `bash ./build_android.sh` (Android NDK, `cargo-ndk`) |
| ESP32 | 🟡 Experimental | `bash ./build_esp32.sh` (xtensa toolchain) |
| Windows | 🟡 Cross-build and test only | — |

## Configuration

All settings live in `config.toml` in the working directory; the application will not start without it.

## Architecture

- Context, container, component and sequence diagrams: [`docs/architecture/`](docs/architecture)
  ([overview](docs/architecture/rtp-midi-architecture.md)).
- Architecture decision records: [`adr/`](adr).

### Legacy components

`signaling_server`, `audio_server`, `frontend/`, `ui-frontend/`, `qt_ui/` and the related
WebRTC/WebSocket code are **deprecated** and no longer maintained. New work targets the Android Hub,
the ESP32 visualiser and RTP-MIDI / OSC / mDNS.

## Development

```sh
cargo test --workspace --all-targets
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
bash ./build_all.sh          # build everything
bash ./package_release.sh    # package a release
```

When adding a crate, list it under `[workspace].members` in the root `Cargo.toml`.
See [CONTRIBUTING.md](CONTRIBUTING.md) and the ADRs before major changes.

## Troubleshooting

- **No LEDs light up** — check the WLED/ESP32 IP address, LED count and power.
- **Audio not detected** — check the audio device in `config.toml` and permissions.
- **MIDI not working** — check ports and that the devices can see each other on the network.
- **ESP32 / Android build errors** — see `docs/` and the `build_*.sh` scripts.
