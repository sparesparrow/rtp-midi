# Implementace WebRTC aplikace pro audio streaming a MIDI přenos

Na základě analýzy poskytnutých zdrojových kódů a dokumentace jsem vytvořil ucelené řešení pro WebRTC aplikaci, která umožňuje streamování audia a přenos MIDI dat v reálném čase. Řešení sestává ze tří klíčových komponentů: signalizačního serveru, audio serveru a klientské aplikace.

## Architektura systému

Systém je navržen jako modulární aplikace s těmito hlavními komponenty:

- **Signalizační server**: Spravuje WebSocket spojení, registruje klienty a směruje signalizační zprávy
- **Audio server**: Zpracovává audio streamy a MIDI data od klientů
- **Klientská aplikace**: Komunikuje se serverem a odesílá audio a MIDI data

### Struktura projektu

```
soundsystem-app/
├── Cargo.toml
├── src/
│   ├── main.rs                   # Vstupní bod aplikace
│   ├── signaling_server/         # Signalizační server
│   │   ├── mod.rs
│   │   └── main.rs
│   ├── audio_server/             # Audio server
│   │   ├── mod.rs
│   │   └── main.rs
│   ├── client_app/               # Klientská aplikace
│   │   ├── mod.rs
│   │   └── main.rs
│   ├── audio/                    # Audio subsystém
│   │   ├── mod.rs
│   │   ├── codec.rs              # Audio kodeky (Opus)
│   │   └── device.rs             # Správa zvukových zařízení
│   ├── midi/                     # MIDI subsystém
│   │   ├── mod.rs
│   │   ├── device.rs             # Správa MIDI zařízení
│   │   └── rtp/                  # RTP-MIDI implementace
│   │       ├── mod.rs
│   │       ├── packet.rs         # Struktura RTP paketů
│   │       └── session.rs        # Správa RTP relací
│   └── net/                      # Síťový subsystém
│       ├── mod.rs
│       ├── signaling.rs          # Signalizační protokol
│       └── webrtc.rs             # WebRTC konfigurace
```

## Implementace klíčových komponentů

### 1. [Signalizační server](./signaling_server/main.rs)
- umožňuje registraci audio serverů a klientů, směrování signalizačních zpráv

### 2. [Audio server](./audio_server/main.rs)
- zpracovává připojení od klientů, dekóduje audio streamy a MIDI data

### 3. [Klientská aplikace](./client_app/main.rs)
- generuje testovací audio a přeposílá MIDI zprávy ze vstupních zařízení

### 4. [RTP-MIDI implementace](./midi/rtp/packet.rs)
- umožňuje spolehlivý přenos MIDI dat po síti

### Technologický stack

- **WebRTC** - pro peer-to-peer audio streamy
- **Opus** - pro vysoce kvalitní audio kódování s nízkým zpožděním
- **WebSocket** - pro signalizaci
- **MIDI** - pro přenos hudebních dat

### Rozšíření

Pro produkční nasazení by bylo vhodné implementovat:

- **Autentizaci a autorizaci** - zabezpečení proti neoprávněnému přístupu
- **Šifrování** - zajištění důvěrnosti přenášených dat
- **Víceuživatelské mixování** - pro kolaborativní hudební tvorbu
- **Webové rozhraní** - pro snadnou správu a monitoring

# Rust MIDI/Audio to WLED/LED Service (Android)

## Overview

This project implements a modular, production-grade Rust service for Android that synchronizes MIDI and audio input with addressable LEDs via WLED (JSON API, DDP). It supports:
- RTP-MIDI input (via network, e.g., from MIDI Hub)
- Audio-reactive LED control (FFT, mapping, DDP output)
- WLED preset/effect control (JSON API)
- Android integration via AIDL (service + UI)

## Architecture

- **Rust core**: Modular, idiomatic Rust (see `src/`)
  - `audio_input.rs`: Audio capture (cpal)
  - `audio_analysis.rs`: FFT, feature extraction (rustfft)
  - `light_mapper.rs`: Audio→LED mapping
  - `ddp_output.rs`: DDP output (ddp-rs)
  - `wled_control.rs`: WLED JSON API
  - `config.rs`: Loads config.toml
  - `android/aidl_service.rs`: AIDL service stub
- **Android UI**: Minimal Kotlin/Java app (in progress)
- **Tasker path**: See `tasker/README.md` (TODO)

## Configuration

All user-configurable parameters are in `config.toml`:
- `wled_ip`: IP address of WLED controller
- `ddp_port`: UDP port for DDP (default: 4048)
- `led_count`: Number of LEDs
- `color_format`: "RGB" or "RGBW"
- `audio_device`: Name of audio input device (optional)
- `test_midi_port`: RTP-MIDI port (default: 5004)
- `log_level`: Logging verbosity

## Android Integration (AIDL)

- The Rust service exposes an AIDL interface (`IMidiWledService.aidl`) for control from Android UI or other apps.
- Methods: `startListener`, `stopListener`, `setWledPreset`, `getStatus`
- See `src/android/aidl_service.rs` for the Rust stub.
- The Android UI app (in progress) binds to this service for user control.

## Tasker Automation Path (Prototype)

- See `tasker/README.md` for rapid prototyping using Tasker, MIDI Hub, and plugins.
- This is a TODO; the production path is the Rust/AIDL service.

## Build & Deploy

- Cross-compile Rust for Android using the NDK (see Mozilla and AOSP docs)
- Build scripts (`build_android.sh`, `deploy_android.sh`) will automate this process (to be added)
- Deploy via ADB to a connected Android device

## Status

- Rust core modules: scaffolded and ready for integration
- Android AIDL service: stubbed, ready for implementation
- Android UI: in progress
- Tasker path: placeholder

## References
- [Mozilla: Building and Deploying a Rust library on Android](https://mozilla.github.io/firefox-browser-architecture/experiments/2017-09-21-rust-on-android.html)
- [AOSP: Android Rust introduction](https://source.android.com/docs/setup/build/rust/building-rust-modules/overview)
- [WLED JSON API](https://kno.wled.ge/interfaces/json-api/)
- [ddp-rs crate](https://github.com/coral/ddp-rs)

