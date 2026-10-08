# rtp-midi: Modulární architektura pro real-time MIDI, audio a LED

*[English version](README.md)*

## Přehled

Tento projekt je nyní zaměřen na hybridní Android Hub (Kotlin + Rust/NDK) pro nízko-latenční MIDI routing a vizualizaci, s podporou ESP32 LED vizualizéru přes OSC a DAW přes RTP-MIDI. Legacy komponenty (WebRTC signaling server, staré UI frontendy, audio_server) jsou **deprekovány** a přesunuty do archivu. Viz sekce "Deprecation & Legacy Components" níže.

### Klíčové vlastnosti
- **Modulární design**: Oddělené crates pro `core`, `network`, `audio`, `output`, `platform` a hardwarové abstrakce (`hal-*`).
- **Android Hub**: Hybridní aplikace (Kotlin UI + Rust NDK core) s foreground service, AMidi NDK, mDNS discovery, dual-protocol (RTP-MIDI pro DAW, OSC pro ESP32).
- **ESP32 vizualizér**: Arduino Core, FastLED, dual-core FreeRTOS, OSC server, konfiguračně řízený hardware.
- **Moderní CI/CD**: Automatizované testování, lintování, bezpečnostní audity, releasy, nasazení na GitHub Pages a publikace Docker image do GHCR.
- **Konfigurovatelnost**: Všechna nastavení jsou spravována přes `config.toml`.

---
## Obsah
1.  [Stav projektu](#stav-projektu)
2.  [Getting Started](#getting-started)
3.  [Platform Support & Building](#platform-support--building)
4.  [Architektura a design](#architektura-a-design)
5.  [Deprecation & Legacy Components](#deprecation--legacy-components)
6.  [Troubleshooting](#troubleshooting)
7.  [Contributing](#contributing)

---

## 1. Stav projektu

### Architektura
- Architektonické diagramy v `docs/architecture/` jsou aktuální.
- ADRs v `adr/` dokumentují klíčová rozhodnutí.

### Stav migrace na novou architekturu

| Oblast | Hotovo | Zbývá |
|---|:---:|:---:|
| Struktura workspace (core, audio…) | ✅ | – |
| `hal-*`, `service-bus` crates | ✅ | – |
| Přesun modelů do `core` | ✅ | – |
| ADR konsolidace & update | ✅ | – |
| CI audit + deny | 🟡 | doladit `deny.toml` |
| Jednotný shutdown | ✅ | – |
| FFI pravidla v CI | ✅ | – |
| AppleMIDI handshake+CK | ✅ | – |
| DDP receiver | ✅ | – |

---
## 2. Getting Started

### Požadavky
- Rust (latest stable, viz [rustup.rs](https://rustup.rs))
- Pro Android: Android NDK, `cargo-ndk`
- Pro ESP32: xtensa toolchain (viz `docs/`)
- Pro Docker: Docker nebo kompatibilní container runtime

### Rychlý start (Linux)
```sh
git clone https://github.com/sparesparrow/rtp-midi.git
cd rtp-midi
# # config.toml je součástí repozitáře # Volitelně upravte
cargo run --release --bin rtp_midi_node -- --role server
```

### Spuštění přes Docker
Projekt lze také spustit v Docker kontejneru. Image jsou automaticky publikovány v [GitHub Container Registry](https://github.com/sparesparrow/rtp-midi/pkgs/container/rtp-midi).

```sh
# Stáhnout a spustit nejnovější verzi
docker run -it --rm -p 5004:5004/udp ghcr.io/sparesparrow/rtp-midi:latest
```

---
## 3. Platform Support & Building

### Nativní build
- **Linux:** Plně podporováno. `cargo build --release`
- **Android:** Podporováno. `bash ./build_android.sh`
- **ESP32:** Experimentální. `bash ./build_esp32.sh`

### Web UI
Webové rozhraní je v `ui-frontend/` a je automaticky nasazováno na GitHub Pages.

### Containerization (Docker)
K dispozici je `Dockerfile` pro sestavení a spuštění aplikace v izolovaném prostředí.
```sh
# Lokální sestavení Docker image
docker build -t rtp-midi-local .

# Spuštění lokálně sestavené image
docker run -it --rm -p 5004:5004/udp rtp-midi-local
```
---
## 4. Architektura a design

- **Android Hub**: Hybridní aplikace (Kotlin UI + Rust NDK core), foreground service, AMidi NDK, mDNS discovery, dual-protocol (RTP-MIDI pro DAW, OSC pro ESP32).
- **ESP32 vizualizér**: Arduino Core, FastLED, dual-core FreeRTOS, OSC server, konfiguračně řízený hardware.
- **Deprecation**: Legacy komponenty (WebRTC signaling server, staré UI frontendy, audio_server) jsou deprekovány a přesunuty do archivu.

### Diagrams
- Kontextové, kontejnerové a komponentové diagramy jsou v `docs/architecture/`.
- Vizualizují aktuální datové toky: Maschine → Android Hub (USB MIDI) → [RTP-MIDI → DAW] + [OSC → ESP32 → LED].

---
## 5. Deprecation & Legacy Components

**Deprecation Notice:**
- Komponenty `signaling_server`, `audio_server`, `frontend/`, `ui-frontend/`, `qt_ui/` a související WebRTC/WebSocket logika jsou **deprekovány** a nejsou dále udržovány.
- Tyto části byly přesunuty do archivu nebo označeny jako legacy. Nový vývoj je zaměřen výhradně na Android Hub, ESP32 vizualizér a moderní protokoly (RTP-MIDI, OSC, mDNS).
- Dokumentace k těmto komponentám je dostupná v archivu pro referenci.

---
## Configuration

The application is configured via `config.toml` in the working directory. You can copy `config.toml.example` to `config.toml` and edit as needed:



### Docker Usage with Custom Config
To use a custom config with Docker:
```sh
docker run -it --rm -v "$PWD/config.toml:/app/config.toml" -p 5004:5004/udp rtp-midi-local
```

---
## Troubleshooting
- **Missing config.toml:** Ensure `config.toml` is present in the working directory or mounted into Docker. The app will fail to start if missing.
- **Workspace errors:** If you add new crates, update the `[workspace].members` in the root `Cargo.toml`.

---
## Running Tests and CI Locally
- Run all tests: `cargo test --workspace --all-targets`
- Lint: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
- Build all: `bash ./build_all.sh`
- Package for release: `bash ./package_release.sh`

---
## 6. Troubleshooting

- **No LEDs light up:** Check WLED IP, LED count, and power.
- **Audio not detected:** Verify audio device in config and permissions.
- **MIDI not working:** Ensure correct ports and network visibility.
- **Build errors (ESP32/Android):** See platform-specific docs in `docs/` and `build_*.sh` scripts.
- **UI not updating:** Reload page, check browser console for errors.

---
## 7. Contributing

- See ADRs and architecture docs before major changes.
- Follow modular, testable, idiomatic Rust practices.
- All config should be externalized; document new options.

---

# FAQ

- **No LEDs light up:** Check WLED IP, LED count, and power.
- **Audio not detected:** Verify audio device in config and permissions.
- **MIDI not working:** Ensure correct ports and network visibility.
- **Build errors (ESP32/Android):** See platform-specific docs in `docs/` and `build_*.sh` scripts.
- **UI not updating:** Reload page, check browser console for errors.

---

# Platform Support Table

| Platform | Status | Build | Notes |
|----------|--------|-------|-------|
| Linux    | ✅     | Native| Full support |
| Android  | ✅     | NDK   | Hybrid Kotlin+Rust, AMidi NDK |
| ESP32    | ✅     | Arduino/PIO | FastLED, OSC |
| Windows  | 🟡     | Cross | Build/test only |

# Diagrams

(Insert latest diagrams from `docs/architecture/`)

---

# Changelog

- 2024-06: Refocused project on Android Hub/ESP32/dual-protocol architecture. Legacy components deprecated.
- 2024-06: Unified shutdown, error handling, and modularity completed.
- 2024-06: CI/CD, Docker, and documentation updated.

---

# (All TODOs and roadmap items that are now implemented or obsolete have been removed. For historical roadmap, see `rtp-midi Project Implementation Tasks.md`.)

