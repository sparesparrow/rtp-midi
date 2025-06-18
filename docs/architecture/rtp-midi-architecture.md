# RTP-MIDI Rust Project Architecture (Updated)

## Overview

The rtp-midi project is a modular, event-driven Rust workspace for real-time MIDI over network, audio-to-LED sync, and cross-platform integration (WLED/ESP32, Android, Qt/Web UI).

### Core Principles
- **Event-Driven Architecture**: All major components communicate via an internal EventBus, decoupling logic and enabling modularity and testability.
- **Modular Crate Structure**: Each major function (protocol, audio, output, platform, UI, HAL) is a separate crate or module.
- **FFI Bridge**: The `platform` crate provides a safe FFI boundary for Qt/Android/Web integration.

## Main Crates/Modules
- `core`: Protocol logic, event bus, session manager, packet processor, journal engine, config.
- `audio`: Audio input and analysis.
- `output`: DDP, WLED, light mapping.
- `network`: MIDI, RTP, AppleMIDI, session management.
- `crates/hal-*`: Platform-specific hardware abstraction (Android, ESP32, PC).
- `crates/service-bus`: Service bus for inter-crate communication.
- `ui-frontend`: Web/Qt UI frontend.
- `platform`: FFI bridge for UI and external integration.

## Event Flow
- MIDI/audio/network input triggers events on the EventBus.
- SessionManager manages AppleMIDI/RTP sessions and clock sync.
- PacketProcessor parses RTP-MIDI and recovery journal.
- JournalEngine handles loss recovery and state repair.
- Output modules map events to DDP/WLED/LED output.
- FFI bridge exposes core events and controls to UI/Android/Qt.

## Modularity & Extensibility
- Each crate is independently testable and replaceable.
- New platforms (e.g., Ableton Link, MIDI 2.0) can be added as crates.
- Unified logging, config, and error handling across workspace.

## See also
- [component_diagram.mmd](component_diagram.mmd)
- [container_diagram.mmd](container_diagram.mmd)
- [context_diagram.mmd](context_diagram.mmd)
- [sequence_diagram.mmd](sequence_diagram.mmd) 