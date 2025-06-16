---
description: MIDI, ESP32, and Rust on Android: integration, WLED, and modularity rules
globs: src/android/aidl_service.rs, src/midi/rtp/packet.rs, src/wled_control.rs
---
# Cursor Rules: MIDI, ESP32, and Rust on Android

## 1. Architectural Path Selection
- Offer both Automation-Centric (Tasker-based) and Service-Oriented (Rust/AIDL-based) solutions in code and documentation. If the user query is ambiguous, present both options.
- Default to the Service-Oriented (Rust/AIDL) path for production-grade, low-latency, and robust solutions, unless rapid prototyping is explicitly requested.

## 2. MIDI Input Handling
- Prioritize RTP-MIDI (rtpmidi crate) for MIDI ingestion in Rust services. Only use direct USB MIDI (rusb/libusb) if explicitly required and document the increased complexity.
- For Android MIDI input, recommend using a network bridge app (e.g., MIDI Hub) to forward USB MIDI to RTP-MIDI, unless the user requests direct USB access.

## 3. WLED Control
- Use the WLED JSON API for all light control commands. Prefer the wled-json-api-library crate for type safety, but allow for raw reqwest/serde_json if minimal dependencies are desired.
- All HTTP requests to WLED must be POSTs to /json/state with a properly constructed JSON body. Validate payloads against the WLED API documentation.

## 4. Android Integration
- For Rust/Android integration, use UniFFI or AIDL for FFI and IPC, not manual JNI unless absolutely necessary.
- All AIDL interfaces must be defined in .aidl files and placed in the correct src/main/aidl/ directory structure.
- Rust services must register with the Android Service Manager using a unique name and implement the generated AIDL trait.

## 5. User Interaction
- If a UI is required, provide a simple Kotlin/Java client that binds to the Rust service via AIDL and exposes the service methods.
- For voice control, recommend cloud-bridged solutions (e.g., Voice Monkey + IFTTT + HTTP webhook) for complex commands, and local emulation (FauxmoESP) for simple on/off/brightness.

## 6. Modularity and Extensibility
- Keep MIDI ingestion, WLED control, and Android IPC as separate modules with clear interfaces.
- All code must be documented and structured for easy extension (e.g., adding new MIDI-to-light mappings, supporting new lighting protocols).

## 7. Testing and Debugging
- Include unit and integration tests for all Rust modules, especially for MIDI parsing and WLED command generation.
- Log all incoming MIDI events and outgoing WLED commands for debugging.
- Provide troubleshooting steps for common issues (e.g., network, permissions, device discovery).

## 8. General Cursor Rules
- If a user query is ambiguous, always offer alternatives and clarify the options.
- Always follow official documentation and best practices for all third-party crates and APIs.
- All code must be idiomatic Rust, with clear error handling and documentation.
- Never assume direct hardware access unless explicitly stated; always clarify the role of intermediary controllers (e.g., WLED, ESP32). 