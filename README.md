# rtp-midi: Modulární architektura pro real-time MIDI, audio a LED

Tento projekt využívá idiomatickou architekturu Rust workspace s oddělením core logiky, platformních HAL vrstev a samostatného UI. Všechny síťové odesílače a přijímače implementují sjednocené traity `DataStreamNetSender` a `DataStreamNetReceiver`.

## Nové crates v projektu

- **hal-pc**: PC HAL adapter, připravený pro platformně specifické implementace výstupů a vstupů.
- **hal-esp32**: ESP32 HAL adapter, připravený pro embedded buildy a statickou dispatch.
- **hal-android**: Android HAL adapter, připravený pro mobilní buildy.
- **service-bus**: Abstrakce pro asynchronní message passing mezi komponentami (tokio broadcast event bus).

## Klíčové principy architektury

- **Monorepo workspace**: Všechny crates jsou spravovány centrálně v root `Cargo.toml`.
- **Feature flagy**: Build-time selekce platforem (`hal_pc`, `hal_esp32`, `hal_android`, `ui`).
- **Modulární crates**:
    - `core`: Platformně nezávislá logika, traity, protokoly, no_std.
    - `hal-*`: Platform-specific implementace (PC, Android, ESP32).
    - `service-bus`: Asynchronní message passing (tokio broadcast event bus).
    - `ui-frontend`: Oddělené UI (WASM/Tauri, WebSocket API).
- **Jediný binární entrypoint**: `rtp-midi-node.rs` autodetekuje roli a platformu za běhu.
- **Oddělené UI**: Samostatný crate, komunikace přes WebSocket.
- **CI/CD workflowy**: Automatizace buildů, testů, auditů.

## Přehledná struktura workspace

```mermaid
graph TD
    subgraph Root["rtp-midi/"]
        A("Cargo.toml<br/><i>(Workspace & feature flags)</i>")
        B(crates/)
        C(bin/)
        D(.github/workflows/)
    end
    A --> B
    A --> C
    subgraph Crates["crates/"]
        B1("<b>core</b><br/><i>#![no_std] RTP-MIDI logic<br/>DataStream traits</i>")
        B2("<b>service-bus</b><br/><i>Async services & broadcast event bus</i>")
        B3("<b>hal-pc</b><br/><i>cfg(feature = 'hal_pc')</i>")
        B4("<b>hal-android</b><br/><i>cfg(feature = 'hal_android')</i>")
        B5("<b>hal-esp32</b><br/><i>cfg(feature = 'hal_esp32')</i>")
        B6("<b>ui-frontend</b><br/><i>WASM/Tauri, cfg(feature = 'ui')</i>")
    end
    B --> B1
    B --> B2
    B --> B3
    B --> B4
    B --> B5
    B --> B6
    subgraph Binaries["bin/"]
        C1("<b>rtp-midi-node.rs</b><br/><i>Single binary entrypoint<br/>Role detection logic</i>")
    end
    C --> C1
    subgraph CI_CD["workflows/"]
        D1("ci.yml")
        D2("release.yml")
        D3("audit.yml")
    end
    D --> D1 & D2 & D3
    style Root fill:#f9f,stroke:#333,stroke-width:2px
    style Crates fill:#ccf,stroke:#333,stroke-width:2px
    style Binaries fill:#cfc,stroke:#333,stroke-width:2px
    style CI_CD fill:#fec,stroke:#333,stroke-width:2px
```

## Spuštění hlavního binárního souboru

Hlavní binární soubor `rtp-midi-node` lze spustit ve třech režimech podle role:

- **Server:**
  ```sh
  cargo run --bin rtp-midi-node -- --role server
  ```
- **Client:**
  ```sh
  cargo run --bin rtp-midi-node -- --role client
  ```
- **UI Host (webserver pro WASM UI):**
  ```sh
  cargo run --bin rtp-midi-node -- --role ui-host
  ```

Každý režim spouští odpovídající službu podle autodetekce role.

---

**Poznámka:**
- Všechny hlavní TODO body pro architekturu, event bus a asynchronní message passing jsou nyní implementovány.
- Pro rozšíření mappingů o další typy akcí/výstupů přidejte nový enum do utils, implementujte nový sender a rozšiřte service loop.
- Pro další informace viz dokumentaci v `docs/architecture/`.

---

## Remaining Technical TODOs and Issues

Below is a summary of outstanding TODOs and technical issues found in the codebase, with their locations and a brief description. These should be addressed in future development cycles:

- **Tasker Automation Path** (`tasker/README.md`):
  - The Tasker-based automation path is a placeholder and not implemented.

- **Data Channel Handling in UI** (`frontend/script.js`):
  - Indicate when the data channel is ready for MIDI data.
  - Process incoming MIDI data on the data channel.
  - Handle data channel closure events.

- **AppleMIDI Handshake and Clock Sync** (`core/src/session_manager.rs`):
  - Implement the full AppleMIDI handshake and clock synchronization state machine.

- **RTP-MIDI Session** (`network/src/midi/rtp/session.rs`):
  - Map `MidiMessage` to `TimedMidiCommand` for journaling.
  - Implement parsing/handling according to the specific format.

- **DDP Receiver Implementation** (`output/src/ddp_output.rs`):
  - Initialize the DDP receiver (e.g., open socket).
  - Implement reading data from the DDP stream.

- **Release Automation** (`.github/workflows/release.yml`):
  - Add release notes and finalize the release workflow.

- **Audio Input** (`audio/src/audio_input.rs`):
  - Handle unsupported sample formats in a more robust way (currently uses `todo!()`).

- **Entrypoint Improvements** (`rtp_midi_node/src/main.rs`):
  - Add a better webserver or Tauri integration for UI hosting.
  - For embedded/ESP32 builds, autodetect platform via feature flags or environment variables.

- **UI Frontend** (`ui-frontend/README.md`):
  - Contains a TODO section for further UI/UX improvements.

---

## LED Mapping Presets

The system supports multiple LED mapping modes, selectable at runtime via the config file:

- `mapping_preset = "spectrum"` (default): Maps audio spectrum to LED colors using a hue gradient.
- `mapping_preset = "vumeter"`: Lights up LEDs as a VU meter based on average audio level.

To change the mapping mode, set the `mapping_preset` field in `config.toml` to either `spectrum` or `vumeter`.

---

## End-to-End Integration Testing

The project includes automated end-to-end integration tests that simulate the full workflow from audio input to LED output, covering both supported mapping presets. These tests ensure that the system produces correct LED data for given audio input and that all major data flows are exercised.

To run all tests:

```sh
cargo test --all --workspace
```

---

## User-Configurable Settings in UI

The web UI now includes a Settings panel (⚙️ button) that allows users to configure:
- **LED Count**: Number of LEDs to control (default: 60)
- **Mapping Preset**: LED mapping mode (`spectrum` or `vumeter`)

Settings are saved in your browser's local storage and persist across reloads. Changes take effect immediately in the UI.

---

## Planned TODOs for Future Development

Below are prioritized tasks for future development. Each TODO includes clear instructions and acceptance criteria.

### 1. Documentation Polish & Examples
- **Instructions:**
  - Expand documentation with usage examples, diagrams, and troubleshooting tips.
- **Acceptance Criteria:**
  - README and docs are comprehensive and up to date.

### 2. Release Automation & Packaging
- **Instructions:**
  - Add scripts or CI jobs for building and packaging releases for all platforms (Linux, Android, ESP32).
- **Acceptance Criteria:**
  - Releases are reproducible and easy to install.

### 3. CI/CD and Release Automation
- **Instructions:**
  - Implement and maintain workflows in `.github/workflows/` for:
    - Automated builds for all platforms
    - Automated tests and linting
    - Automated release creation with release notes and artifacts
    - Automated code reviews and test results reviews using LLM APIs called by GitHub Actions
- **Acceptance Criteria:**
  - All builds, tests, and releases are automated and reproducible
  - Code and test reviews are enhanced by LLM-based automation

### 4. Code Quality & Static Analysis
**Instructions:**
- Add `clippy` and `rustfmt` checks to the workflow in `.github/workflows/` and make the build fail on warnings.
- Configure `cargo deny` to scan for vulnerable, unmaintained or duplicate dependencies.
**Acceptance Criteria:**
- PRs are blocked if `clippy`, `rustfmt` or `cargo deny` report findings.
- Summary of the three tools is shown in the GitHub Checks tab.

### 5. Unified Logging & Tracing
**Instructions:**
- Introduce the `tracing` crate in `core` and all `hal-*` crates; export a reusable `init_tracing()` helper behind the `log` feature flag.
- Route logs to the browser console when running in WASM and to `systemd-journal` when on Linux PC.
**Acceptance Criteria:**
- Every major state-machine transition (RTP-MIDI session, AppleMIDI handshake, audio pipeline) emits structured spans visible in `tokio-console` or `tracing-flame`.

### 6. Fuzzing & Property-Based Tests
**Instructions:**
- Add a `fuzz/` directory and integrate `cargo-fuzz` for parsers in `network/src/midi/rtp/session.rs`.
- Use `proptest` for serialization/deserialization round-trip tests of MIDI and LED frames.
**Acceptance Criteria:**
- CI runs at least one minute of fuzzing on every push.
- Round-trip tests reach > 95 % branch coverage (measured with `grcov`).

### 7. Performance Optimisation of LED Mapping
**Instructions:**
- Replace per-LED loop in `mapping_spectrum.rs` with SIMD using `wide` or `packed_simd_2`.
- Benchmark before/after with `criterion` on x86-64 and ESP32 (Xtensa).
**Acceptance Criteria:**
- Throughput improvement ≥ 2× on x86-64 and ≥ 1.3× on ESP32 without raising binary size by more than 5 %.

### 8. Packaging & Distribution
**Instructions:**
- Provide `Dockerfile` that builds a minimal image (~25 MB) with `rtp-midi-node` and default config.
- Create `deb` and Homebrew formulas via `cargo-deb` and `brew tap`.
**Acceptance Criteria:**
- Tagged GitHub release automatically uploads `.deb`, Homebrew bottle and Docker image to GHCR.

### 9. OTA Update Flow for ESP32
**Instructions:**
- Add an HTTP-based OTA endpoint guarded by a simple token to the `hal-esp32` runtime.
- Document update steps in `docs/platforms/esp32.md`.
**Acceptance Criteria:**
- Full firmware update succeeds in < 30 s over Wi-Fi with progress logged to UART and the web UI.

### 10. Ableton Link Synchronisation
**Instructions:**
- Introduce optional `ableton_link` feature using the `ableton-link` crate; expose tempo and phase on the `service-bus`.
- Visualise Link status in the web UI header.
**Acceptance Criteria:**
- When another Link peer is present, tempo lock jitter < 0.5 BPM (measured over 5 min).

### 11. Documentation Site via mdBook
**Instructions:**
- Generate an mdBook in `docs/book/` from existing ADRs and architecture diagrams; deploy with GitHub Pages from `gh-pages` branch.
- Link the book prominently from the README title section.
**Acceptance Criteria:**
- `docs.rs` badge and "Open Book 📖" link appear in README; Pages site builds without warnings.

### 12. README Maintenance Task
**Instructions:**
- Move each completed item from "Planned TODOs" to a new "Changelog of Completed Tasks" subsection to keep the list concise.
- Renumber the remaining TODOs sequentially after every merge.
**Acceptance Criteria:**
- `Planned TODOs` never exceeds 15 open items; changelog shows date, PR number and contributor for each moved task.

---

# Getting Started

## Prerequisites
- Rust (latest stable, see [rustup.rs](https://rustup.rs))
- For Android: Android NDK, cargo-ndk
- For ESP32: xtensa toolchain (see docs/)
- For UI: modern web browser

## Quick Start (Linux)
```sh
git clone https://github.com/sparesparrow/rtp-midi.git
cd rtp-midi
cargo build --release
cp config.toml.example config.toml # Edit as needed
cargo run --release --bin rtp_midi_node -- --role server
```

## Running the Web UI
- Open `frontend/index.html` in your browser, or run the backend in `--role ui-host` mode to serve it.

---

# Usage Examples

## Audio to LED (WLED)
- Connect a microphone or audio source.
- Configure `wled_ip`, `led_count`, and `mapping_preset` in `config.toml`.
- Start the service. LEDs will sync to audio in real time.

## MIDI over RTP
- Use a compatible MIDI client to connect to the server.
- MIDI messages are routed and can be visualized in the UI.

## UI Settings
- Use the ⚙️ Settings panel in the web UI to adjust LED count and mapping preset at runtime.

---

# Configuration Summary

All options are in `config.toml`:
- `wled_ip`: IP address of your WLED controller
- `led_count`: Number of LEDs
- `mapping_preset`: `spectrum` or `vumeter`
- (See file for more options)

UI settings (LED count, mapping) are stored in your browser and override config at runtime.

---

# Architecture & Diagrams

- See `docs/architecture/` for context, container, and sequence diagrams.
- ADRs in `adr/` document key design decisions.

---

# Troubleshooting
- **No LEDs light up:** Check WLED IP, LED count, and power.
- **Audio not detected:** Verify audio device in config and permissions.
- **MIDI not working:** Ensure correct ports and network visibility.
- **Build errors (ESP32/Android):** See platform-specific docs in `docs/` and `build_*.sh` scripts.
- **UI not updating:** Reload page, check browser console for errors.

---

# Platform Support & Building
- **Linux:** Native, fully supported.
- **Android:** Build with `build_android.sh` (requires NDK).
- **ESP32:** See `build_esp32.sh` and docs for toolchain setup.

---

# Contributing
- See ADRs and architecture docs before major changes.
- Follow modular, testable, idiomatic Rust practices.
- All config should be externalized; document new options.

---

