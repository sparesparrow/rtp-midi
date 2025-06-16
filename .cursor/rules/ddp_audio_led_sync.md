---
description: DDP audio-LED sync rules for Rust project, including protocol, analysis, and modularity
globs: src/audio_input.rs, src/audio_analysis.rs, src/light_mapper.rs, src/ddp_output.rs, src/config.rs
---
# Cursor Rules: DDP Audio-LED Sync Rust Project

## 1. Protocol and Hardware Alignment
- All DDP packets must use a 10-byte header (omit timecode for WLED).
- Default UDP port for DDP is 4048.
- DDP payload must match WLED's LED configuration (number of pixels, RGB/RGBW, color order).

## 2. Audio Input and Analysis
- Use `cpal` for audio input (cross-platform, low-latency).
- Use `rustfft` for FFT; prefer `FftPlanner` for runtime flexibility.
- Normalize and smooth all audio features before mapping to light parameters.
- Implement basic mappings first (e.g., bass to red, amplitude to brightness), and structure code for easy addition of advanced mappings.

## 3. Real-Time and Multi-Threading
- Separate audio capture, analysis, and DDP output into distinct threads or async tasks.
- Use channels (`std::sync::mpsc` or `crossbeam`) for inter-thread communication.
- Regulate DDP frame rate (e.g., 30–60 FPS) using timers or sleep, ensuring the pipeline completes within the frame budget.

## 4. Configuration and Logging
- All user-configurable parameters (e.g., DDP target IP, LED count, audio device) must be loaded from a config file (TOML or JSON, using `serde`).
- Log all errors, warnings, and key events using the `log` crate and a concrete backend (`env_logger` or `fern`).

## 5. Testing and Validation
- Write unit tests for FFT, RMS, and mapping functions.
- Provide integration tests for the full pipeline (audio in → DDP out).
- Include tools or scripts for capturing and inspecting DDP packets (e.g., Wireshark instructions).

## 6. Documentation and User Guidance
- Document the distinction between "analog" and "digital" LED strips and clarify that the Rust app only sends DDP to a controller (e.g., WLED), not directly to hardware.
- Provide setup instructions for WLED (IP config, LED count, color order).
- If ambiguity exists in user queries (e.g., RGB vs. RGBW), offer both options and let the user select.

## 7. Modularity and Extensibility
- Organize code into modules: `audio_input`, `audio_analysis`, `light_mapper`, `ddp_output`, `config`, `main`.
- Design mapping logic to be easily extensible (e.g., trait-based or function-pointer-based mapping strategies).

## 8. General Cursor Rules
- If a user query is ambiguous, always offer alternatives and clarify the options.
- Always follow official documentation and best practices for all third-party crates and APIs.
- All code must be idiomatic Rust, with clear error handling and documentation.
- Never assume direct hardware access unless explicitly stated; always clarify the role of intermediary controllers (e.g., WLED, ESP32). 