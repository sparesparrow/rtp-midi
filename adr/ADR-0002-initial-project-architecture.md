# ADR-0002: Initial Project Architecture

## Status

Accepted

## Context

The `rtp-midi` project aims to provide real-time audio analysis and LED synchronization, utilizing the DDP protocol to control LED strips via WLED/ESP32. The core logic is implemented in Rust, emphasizing performance, reliability, modularity, and cross-platform compatibility. The project integrates various frontends and communication mechanisms.

## Decision

The initial architecture for the `rtp-midi` project will consist of a central Rust backend responsible for core logic, communicating with different frontends via FFI (for desktop UI) and WebSockets/WebRTC (for web frontend). Android integration will use JNI/AIDL. The project will be structured as a Cargo workspace to promote modularity.

## Consequences

*   **Positive:**
    *   Leverages Rust's strengths for performance, reliability, and memory safety, crucial for real-time audio and network operations.
    *   Modular design with a Cargo workspace facilitates independent development, testing, and clearer component boundaries.
    *   Support for multiple frontends (Qt Desktop, Web, Android) enhances usability and reach.
    *   Clear separation of concerns between core logic and UI layers.
*   **Negative:**
    *   Requires careful management of FFI and JNI boundaries to ensure safety and prevent panics.
    *   Initial setup and configuration of the multi-platform build environment may be complex.
    *   Synchronization and data flow across different communication channels need robust design to maintain real-time performance. 