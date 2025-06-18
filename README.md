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

1. Core Logic & Protocol Implementation
 * Instructions:
   * Implement the full AppleMIDI handshake and clock synchronization state machine in network/src/midi/rtp/session.rs and core/src/session_manager.rs. The current implementation is a placeholder. The session should not be considered "established" until both sides have completed the IN, OK, and CK message exchange.
   * [x] Implement the recovery journal retransmission logic. The session now detects gaps in sequence numbers in RtpMidiSession::handle_incoming_packet and attempts to recover missing packets using the journal mechanism, logging the outcome for each gap.
   * Complete the DDP (Distributed Display Protocol) receiver implementation in output/src/ddp_output.rs. The DdpReceiver struct is currently a stub and its poll method should be implemented to read data from a UDP socket.
   * [x] Fully integrate audio analysis with the output modules in rtp_midi_lib/src/lib.rs. The main service loop now sends led_data to the active DDP sender, so LED output reflects real-time audio analysis.
 * Acceptance Criteria:
   * A new RTP-MIDI peer connection correctly performs the two-way handshake (IN/OK) and clock synchronization (CK0, CK1, CK2) before processing MIDI data.
   * The system can detect and recover from lost packets using the recovery journal mechanism.
   * The application can receive and process incoming DDP data.
   * Real-time audio analysis is visibly reflected on the configured LED output (WLED or DDP).

2. Architecture & Refactoring
 * Instructions:
   * Refactor shared data structures (MidiCommand, Mapping, InputEvent, WledOutputAction, etc.) from utils/src/lib.rs and rtp_midi_lib/src/lib.rs into the rtp_midi_core crate. The goal is to make rtp_midi_core the single source of truth for all platform-agnostic data models. The utils crate can then be deprecated or merged if it becomes redundant.
   * Create a unified and robust shutdown mechanism. Replace thread_handle.abort() in platform/src/ffi.rs with a graceful shutdown signal (e.g., using a tokio::sync::watch channel). The main service loop and all spawned tasks should listen for this signal and terminate cleanly.
   * Remove duplicated frontend and ui-frontend files from the dist/ directory in the source control. The package_release.sh script should be the only component responsible for copying these files into the release package, not Git.
 * Acceptance Criteria:
   * All core data structures are defined within the rtp_midi_core crate. Other crates import them from there.
   * Calling stop_service from the FFI or sending a Ctrl+C signal results in all threads and async tasks shutting down without panics or abrupt termination.
   * The git status command shows no duplicated frontend files; the dist/ directory is clean.

3. Error Handling & Robustness
 * Instructions:
   * Replace all uses of .unwrap() and .expect() in application logic (especially in rtp_midi_lib/src/lib.rs and server binaries) with structured error handling using anyhow::Result and the log crate. Panics should only occur for unrecoverable state errors.
   * Standardize error logging. All errors captured via Result or in catch blocks should be logged using log::error!, instead of eprintln!, for consistency.
   * Handle potential failures in the audio/src/audio_input.rs module. Specifically, replace the todo!("Unsupported sample format") with a fallback mechanism or a clear error message that informs the user which formats are supported.
 * Acceptance Criteria:
   * The application is resilient to common failures (e.g., file not found, network issues) and logs descriptive errors instead of panicking.
   * The application produces no output on stderr during normal operation; all diagnostic messages go through the log crate.
   * The application provides a clear error message if an unsupported audio device or sample format is selected.

4. Build & CI/CD
 * Instructions:
   * Automate the generation of the dependency graph. Create a new GitHub Actions workflow step that runs cargo tree --workspace and commits the updated docs/dep-graph.txt file to the repository.
   * The build_android.sh script currently overwrites .cargo/config.toml. Modify it to check if the file exists and, if so, to merge the required configuration instead of overwriting it, or at least to back up the original file first.
   * Finalize the release automation. In .github/workflows/release.yml, replace the "TODO: Add release notes here" placeholder with a mechanism to automatically generate release notes from commit messages or pull request titles since the last tag.
 * Acceptance Criteria:
   * The docs/dep-graph.txt file is automatically kept in sync with the project's dependencies on every push to master.
   * The Android build script no longer causes data loss by overwriting user configuration files.
   * Creating a new Git tag (e.g., v0.2.0) automatically triggers the release.yml workflow, which creates a GitHub Release with populated release notes and attached build artifacts.

5. Frontend & UI
 * Instructions:
   * Implement the remaining WebRTC data channel logic in frontend/script.js. Specifically, address the TODO comments for indicating when the data channel is ready, processing incoming MIDI data (e.g., visualizing it on the piano), and handling channel closure.
   * Refactor the webrtc_client.html frontend to send MIDI messages via the data channel when piano keys are clicked. The sendMidiMessage function should serialize the MIDI command into a binary format (e.g., Uint8Array) and send it through the dataChannel.
   * Add functionality to the web UI settings panel (frontend/index.html) to send configuration changes (LED Count, Mapping Preset) to the backend via the WebSocket or WebRTC data channel, so they can be applied at runtime.
 * Acceptance Criteria:
   * The web UI correctly establishes a WebRTC peer-to-peer connection with the backend.
   * Clicking a key on the piano in webrtc_client.html sends a valid MIDI Note On/Off message through the data channel, which is received and processed by the Rust backend.
   * Changing the LED count in the UI settings panel and clicking "Save" updates the number of LEDs the backend renders to in real time.

6. Architektura, Správa a Meta-úkoly
 * Instructions:
   * Finalizovat ADR pro modularizaci workspace: `ADR-0003-workspace-modularization.md` je označen jako `Proposed`, ale změny již byly z velké části implementovány. Je potřeba revidovat ADR, aby odrážel finální strukturu `crates/hal-*`, označit jej jako `Accepted` a doplnit datum.
   * Standardizovat názvy ADR souborů a uklidit: V projektu jsou dva ADR pro sledování rozhodnutí (`0000-*.md` a `ADR-0000.md`) a nekonzistentní pojmenování. Je třeba je sjednotit, rozhodnout o jediné šabloně a přejmenovat všechny ADR soubory na konzistentní formát, např. `ADR-XXXX-nazev-slug.md`.
 * Acceptance Criteria:
   * Stav `ADR-0003` je `Accepted` a obsah odpovídá realitě.
   * Všechny ADR soubory v adresáři `adr/` mají konzistentní a jasný název.

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

