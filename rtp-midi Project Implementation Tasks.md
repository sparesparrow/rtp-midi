# **Implementation and Architectural Evolution Plan for rtp-midi**

This report provides a strategic and tactical roadmap for implementing the "Maschine MIDI na ESP32 LED" system by evolving the existing rtp-midi codebase. The objective is to transform the current, versatile but fragmented project into a focused, high-performance audio-visual instrument centered around an Android hub. The analysis begins with a thorough assessment of the project's current state, establishing a baseline for the implementation plan. This is followed by a formal specification of the target architecture derived from the project's goals. The report culminates in a granular, phased implementation plan designed to guide development from foundational protocol enhancements to full system integration and testing.

## **Section 1: Analysis of the Current rtp-midi Ecosystem**

An in-depth assessment of the rtp-midi project reveals a sophisticated, well-structured foundation built on modern Rust principles. The project exhibits a strong commitment to modularity, platform independence, and automated quality assurance. However, this analysis also identifies areas of architectural fragmentation and incomplete features that must be addressed to achieve the focused goal of the "Maschine MIDI na ESP32 LED" system.

### **1.1 Workspace and Crate-Based Architecture**

The project is architected as a Cargo workspace, a design choice that promotes modularity and separation of concerns. This structure is explicitly defined in the root Cargo.toml and documented in the project's README.md and Architecture Decision Records (ADRs). ADR-0004, in particular, codifies the migration to this modular, event-driven architecture, which is a significant strength. The workspace is composed of several key crates, each with a distinct responsibility:

* **Core Logic:** core, rtp\_midi\_lib  
* **Domain-Specific Functionality:** network, audio, output  
* **Platform Abstraction:** platform, hal-pc, hal-esp32, hal-android  
* **Application Entrypoints:** rtp\_midi\_node, client\_app, audio\_server, signaling\_server  
* **Communication:** service-bus

Communication between these components is decoupled via an event bus system. The core crate defines a tokio::sync::broadcast channel in core/src/event\_bus.rs for one-to-many event propagation , while the service-bus crate provides a tokio::sync::mpsc channel for more direct, one-to-one messaging. This event-driven design is a robust foundation for building complex, asynchronous systems.  
Despite this strong modular foundation, the project exhibits a degree of strategic fragmentation. The existence of multiple, disparate binary entry points (rtp\_midi\_node, client\_app, audio\_server, signaling\_server) and several distinct user interface implementations (frontend/, ui-frontend/, qt\_ui/) suggests a history of experimentation without a final, consolidated product vision. The target architecture, as defined in the planning document, specifies a single, powerful "Android Hub" as the central nervous system. This implies that the current general-purpose components, such as the WebRTC-focused signaling\_server and audio\_server, may no longer align with the project's new, more specific direction. The implementation plan must therefore include a clear strategy for consolidating these components. The most logical path forward is to enhance the rtp\_midi\_node crate, leveraging feature flags to enable platform-specific logic, allowing it to fully embody the role of the "Android Hub" when compiled for that target, while deprecating the now-superfluous server components.

### **1.2 Platform Support and CI/CD Infrastructure**

The project's commitment to quality and cross-platform support is evident in its comprehensive Continuous Integration and Continuous Deployment (CI/CD) infrastructure. The primary CI workflow, defined in .github/workflows/ci.yml, is a mature asset that automates building, testing, and quality checks. It builds for all three target platforms: Linux (x86\_64-unknown-linux-gnu), Android (aarch64-linux-android), and ESP32 (xtensa-esp32-none-elf), using a matrix strategy to manage the different configurations. The pipeline enforces a strict zero-warning policy by setting RUSTFLAGS: "-D warnings", ensuring high code quality. Furthermore, it integrates cargo deny for license and dependency auditing and cargo audit for security vulnerability scanning , demonstrating a proactive approach to dependency management.  
The project also features dedicated build scripts for each platform (build\_android.sh, build\_esp32.sh, build\_pc\_linux.sh) and corresponding deployment scripts (deploy\_android.sh, deploy\_esp32.sh, deploy\_pc\_linux.sh), which streamline the development and distribution process. Additional workflows automate the publication of Docker images to the GitHub Container Registry , the deployment of a WASM-based UI to GitHub Pages , and the creation of tagged releases with binary artifacts.  
However, while the CI/CD infrastructure is robust, it has critical gaps concerning the new architectural goals. The CI pipeline explicitly disables tests for the Android and ESP32 targets (run\_tests: false). While this is a common practice to avoid the complexity of setting up emulators or hardware-in-the-loop (HIL) testing, it leaves a significant portion of the platform-specific code untested. Moreover, the new target architecture relies heavily on OSC and mDNS, protocols that are not covered by the current integration tests in tests/integration\_tests.rs, which focus on the WebRTC signaling server and WLED mocking. The implementation plan must therefore extend this strong CI/CD foundation by introducing new, mocked integration tests for the OSC and mDNS protocol layers and adding a build verification step for the new ESP32 firmware binary.

### **1.3 Implemented Communication Protocols**

The current codebase supports several communication protocols, reflecting its history of experimentation and broad feature set.

* **RTP-MIDI:** Core logic for RTP-MIDI sessions exists in network/src/midi/rtp/session.rs. However, the project's README.md explicitly flags the crucial AppleMIDI handshake and clock synchronization mechanisms as incomplete, marked with a yellow status indicator (🟡). This is a critical gap, as a fully compliant handshake is necessary for reliable connectivity with standard DAWs.  
* **DDP (Distributed Display Protocol):** The project has excellent support for DDP. The output/src/ddp\_output.rs file contains a fully implemented DdpSender and DdpReceiver. The README.md confirms that the DDP receiver is complete and functional, marked with a green check (✅).  
* **WebRTC/WebSocket:** A significant portion of the existing code is dedicated to a WebRTC and WebSocket-based communication system. This includes a dedicated signaling\_server binary , an audio\_server that acts as a WebRTC peer , and multiple JavaScript-based frontends designed to interact with this system.

A clear mismatch exists between the currently implemented protocols and the target architecture. The project's heavy investment in a WebRTC/WebSocket stack, while technically sound, is largely irrelevant to the new plan outlined in "Maschine MIDI na ESP32 LED.txt". The new architecture makes no mention of WebRTC, instead specifying a native Android application and a direct OSC link to the ESP32. The primary network protocols for the target system are RTP-MIDI (for the DAW) and OSC (for the ESP32), with mDNS for service discovery. This represents a significant strategic pivot. The implementation should therefore focus entirely on hardening the incomplete RTP-MIDI stack and building the OSC and mDNS layers from the ground up. The existing WebRTC and signaling server components should be formally deprecated to prevent architectural ambiguity and streamline future development.

### **1.4 Frontend Interfaces and FFI Bridge**

The project's history of exploration is also visible in its user interface strategies. The codebase contains at least three distinct frontend approaches: a simple HTML/JavaScript client in the frontend/ directory , a more advanced Yew/WASM application in ui-frontend/ , and a desktop application using Qt/C++ in qt\_ui/.  
To support the Qt UI, a robust Foreign Function Interface (FFI) bridge is implemented in platform/src/ffi.rs. This bridge exposes extern "C" functions and uses an opaque ServiceHandle struct to allow C++ code to manage the lifecycle of the Rust backend safely. For Android, the project is configured for JNI/AIDL integration, with rsbinder included as a workspace dependency and a build.rs script specifically designed to compile .aidl interface files.  
This existing FFI and JNI foundation is a critical asset, demonstrating the project's capability to integrate with native code on other platforms. However, it will require significant extension to meet the new requirements. The target plan calls for direct, low-latency access to USB MIDI devices from the Rust/C++ layer using the Android AMidi NDK API. The current FFI/JNI layer is designed primarily for controlling the service's lifecycle and exchanging simple configuration data, not for the high-performance, low-latency device I/O required by the AMidi API. The implementation plan must therefore include tasks to create new JNI functions capable of passing a USB device handle or file descriptor from the Kotlin/Java layer (where it is obtained via MidiManager) down to the native Rust layer, where it can be used to initialize an AMidi session. This represents a non-trivial but essential extension of the current FFI bridge pattern.

## **Section 2: Specification of the Target Architecture**

This section translates the goals described in the "Maschine MIDI na ESP32 LED.txt" document into a formal architectural specification. This provides a clear and unambiguous vision for the target system, serving as the blueprint for the implementation phase.

### **2.1 System Overview and Core Data Flow**

The target system is an integrated audio-visual instrument centered around an Android device. The core data flow begins with the Native Instruments Maschine controller, which generates MIDI events. These events are sent via USB OTG to the Android Hub application. The Hub application then bifurcates the data stream into two distinct, real-time paths:

1. **Path A (DAW Integration):** MIDI data is encapsulated in RTP-MIDI packets and transmitted over Wi-Fi to a Digital Audio Workstation (DAW) for music production.  
2. **Path B (LED Visualization):** MIDI events are translated into OSC (Open Sound Control) messages and transmitted over Wi-Fi to an ESP32 microcontroller, which drives a real-time LED visualization.

To eliminate ambiguity and define clear boundaries, the responsibilities of each component are specified in the following matrix. This clarifies, for example, that the Android Hub is responsible for protocol translation (MIDI to OSC), while the ESP32 is responsible only for executing visualization logic based on the received OSC commands.

| Functional Area | Maschine Controller | Android Hub | ESP32 Visualizer | DAW |
| :---- | :---- | :---- | :---- | :---- |
| MIDI Event Generation | ✅ |  |  |  |
| USB MIDI Input |  | ✅ (AMidi NDK) |  |  |
| RTP-MIDI Session |  | ✅ (Initiator) |  | ✅ (Participant) |
| OSC Message Generation |  | ✅ (Translator) |  |  |
| OSC Message Reception |  |  | ✅ (Server) |  |
| mDNS Service Publication |  | ✅ (RTP-MIDI) | ✅ (OSC) | ✅ (RTP-MIDI) |
| mDNS Service Discovery |  | ✅ (Finds ESP32) |  | ✅ (Finds Android) |
| LED Visualization Logic |  |  | ✅ (FastLED) |  |
| Audio Synthesis |  |  |  | ✅ |

### **2.2 Protocol Stack Definition**

The system's reliability and performance hinge on the correct implementation of its communication protocols.

* **RTP-MIDI (RFC 6295):** The Android Hub will function as a full, AppleMIDI-compliant session initiator. This requires completing the implementation started in network/src/midi/rtp/session.rs. The implementation must include the two-port handshake (control and data), the three-way clock synchronization exchange (CK0, CK1, CK2) for calculating network latency, and the recovery journal mechanism to ensure resilience against packet loss over Wi-Fi. This is a core requirement for professional use with DAWs. The rtpmidi crate could serve as a reference for a complete implementation.  
* **OSC (Open Sound Control):** For the high-frequency, low-latency link to the ESP32, OSC over UDP will be used. This is a new protocol layer for the project. The rosc crate is a strong candidate for this purpose, offering robust parsing and serialization capabilities. The translation from MIDI events to OSC messages within the Android Hub will follow a well-defined schema, which serves as a firm contract between the Android and ESP32 firmware development efforts.

| MIDI Event | OSC Address | Argument 1 | Argument 2 | Notes |
| :---- | :---- | :---- | :---- | :---- |
| Note On | /noteOn | int note | int velocity |  |
| Note Off | /noteOff | int note |  |  |
| Control Change (CC) | /cc | int controller | int value |  |
| Pitch Bend | /pitchBend | float bendValue | Range \-1.0 to 1.0 |  |
| Program Change | /config/setEffect | int effectId |  |  |

* **mDNS/Bonjour:** To achieve zero-configuration networking, the system will use mDNS for service discovery. The Android Hub will advertise its \_apple-midi.\_udp service for DAWs and simultaneously discover the ESP32's \_osc.\_udp service. The ESP32 will, in turn, advertise its \_osc.\_udp service. For the Rust implementation on Android, the mdns-sd crate is a suitable choice. On the ESP32, the standard ESPmDNS library, part of the Arduino Core, will be used.

### **2.3 Android Hub Component Specification**

The Android Hub application is the most complex component, requiring a hybrid native architecture to meet its performance goals.

* **Architecture:** The application will be built using a **hybrid native model**, combining a Kotlin frontend with a Rust NDK backend. The Kotlin layer will be responsible for the UI (using Jetpack Compose), managing Android Services, handling permissions, and running mDNS discovery via the NsdManager API. The Rust core, invoked via JNI, will handle all real-time MIDI processing, protocol encapsulation, and network I/O.  
* **Low-Latency MIDI:** A critical requirement is the use of the **AMidi NDK API** for MIDI input. This provides direct, low-latency access to the USB MIDI device, bypassing the higher-latency Java MIDI APIs. This approach is essential for achieving the real-time performance expected of a musical instrument.  
* **Foreground Service:** To ensure that MIDI routing continues reliably even when the application is in the background, the core logic must run within a **Foreground Service**. On modern Android versions, this requires specifying the service type as connectedDevice in the AndroidManifest.xml and making the appropriate runtime calls to startForeground().  
* **JNI Bridge:** The existing JNI patterns in platform/src/ffi.rs will be extended. New native methods are required to bridge the gap between the Kotlin and Rust layers, specifically to:  
  1. Pass the native device handle for the USB MIDI device from the Kotlin layer (where it is acquired using MidiManager) to the Rust layer.  
  2. Provide a mechanism for the Kotlin layer to configure the Rust core with the IP address and port of the ESP32, as discovered by mDNS.  
  3. Allow the UI to start and stop the RTP-MIDI and OSC data streams independently.

### **2.4 ESP32 Visualizer Component Specification**

The ESP32 firmware will be a new, dedicated project optimized for real-time LED control.

* **Framework:** The firmware will be developed using the **Arduino Core for ESP32**. This choice is driven by its vast ecosystem of mature libraries, particularly for networking and hardware control, and its underlying use of the FreeRTOS real-time operating system.  
* **LED Control:** All LED control will be managed by the **FastLED library**. It is chosen over Rust-native drivers like ws2812-esp32-rmt-driver due to its superior performance, extensive feature set for color manipulation and animation, and broad support for various LED chipsets, including the target WS2812B.  
* **Firmware Architecture:** A **dual-core, multi-tasking architecture** will be implemented using the FreeRTOS API primitives available within the Arduino Core. This design is crucial for preventing network jitter from impacting visual smoothness.  
  * **Core 0 (Network Task):** This task will be exclusively responsible for handling Wi-Fi connectivity, mDNS advertisements, and the OSC UDP server. Upon receiving an OSC packet, it will parse the message and place a command into a thread-safe queue.  
  * **Core 1 (Animation Task):** This task will run a high-frequency (e.g., 60-120 Hz) rendering loop. In each iteration, it will check the command queue for new messages, update the internal state of the visualization, calculate the next frame of LED colors, and write the data to the LED strip using FastLED.show().  
* **Hardware Abstraction:** A simple but effective **configuration-driven hardware model** will be used. A board\_config.h file will contain all hardware-specific definitions (e.g., LED\_PIN, NUM\_LEDS, LED\_TYPE). This allows the same firmware to be compiled for different ESP32 board layouts simply by changing the configuration file, avoiding code duplication and complex abstraction layers.

## **Section 3: Phased Implementation Roadmap and Task Breakdown**

This section provides a granular, step-by-step development plan. Tasks are logically grouped into four distinct phases, progressing from foundational protocol work to full system integration and refinement. Each task includes a description, a list of key files to be modified, and clear acceptance criteria.

### **Phase 1: Foundational Enhancements & Protocol Implementation**

This initial phase focuses on strengthening the core libraries and implementing the necessary network protocols that are currently missing or incomplete.

* **Task 1.1: Complete AppleMIDI Handshake & Clock Synchronization**  
  * **Description:** Implement the full two-port IN/OK handshake and the three-way CK0/CK1/CK2 clock synchronization for AppleMIDI. The current implementation in network/src/midi/rtp/session.rs and core/src/session\_manager.rs is a placeholder and must be completed to establish reliable sessions with DAWs like Logic Pro and rtpMIDI on Windows. This directly addresses a key TODO from the README.md.  
  * **Files to Modify:** network/src/midi/rtp/session.rs, core/src/session\_manager.rs, network/src/midi/rtp/control\_message.rs.  
  * **Acceptance Criteria:** The Android Hub can successfully establish a session with rtpMIDI on Windows and the native MIDI Network Setup on macOS. Latency and jitter are correctly calculated and reported. The session is stable over long periods.  
* **Task 1.2: Implement a Generic OSC (Open Sound Control) Protocol Layer**  
  * **Description:** Integrate the rosc crate into the rtp-midi workspace. Create a new module, output/src/osc\_output.rs, to handle the serialization of OSC messages according to the schema defined in the *MIDI-to-OSC Message Mapping* table. Implement a new OscSender struct that conforms to the DataStreamNetSender trait defined in core/src/lib.rs to allow for unified output handling within the main service loop.  
  * **Files to Modify/Create:** output/src/osc\_output.rs, core/src/lib.rs, rtp\_midi\_lib/src/lib.rs, output/Cargo.toml.  
  * **Acceptance Criteria:** The Rust core can serialize and send valid OSC packets over UDP to a specified target. Unit tests are created to verify the correct packet formatting for all defined messages (e.g., /noteOn, /cc).  
* **Task 1.3: Integrate mDNS for Service Discovery**  
  * **Description:** Add mDNS capabilities to the Rust core using a crate such as mdns-sd. This will involve creating a new network/src/discovery.rs module. The core service must be able to both advertise its own \_apple-midi.\_udp service for DAWs and discover \_osc.\_udp services broadcast by the ESP32. This discovery logic will need to be exposed to the Android layer via the JNI bridge in platform/src/ffi.rs.  
  * **Files to Modify/Create:** network/src/discovery.rs, platform/src/ffi.rs, network/Cargo.toml.  
  * **Acceptance Criteria:** A running instance of the service can be discovered on the local network using a standard mDNS browser (e.g., Avahi, Bonjour Browser). The service can successfully discover the ESP32's OSC service when it comes online.  
* **Task 1.4: Refactor for a Unified, Graceful Shutdown Mechanism**  
  * **Description:** The README.md identifies a unified shutdown mechanism as a key TODO. The current FFI implementation in platform/src/ffi.rs already establishes a good pattern with a tokio::sync::watch channel. This task involves propagating this shutdown\_rx receiver to all long-running spawned threads and async tasks, including the audio input thread, the network interface listener, and the DDP receiver. Each task must listen for the shutdown signal and terminate cleanly.  
  * **Files to Modify:** rtp\_midi\_lib/src/lib.rs, network/src/network\_interface.rs, audio/src/audio\_input.rs.  
  * **Acceptance Criteria:** Calling stop\_service via FFI or sending a Ctrl+C signal to a standalone binary results in all threads and async tasks shutting down gracefully without panics or resource leaks. Logs should confirm the clean shutdown of each component.

### **Phase 2: ESP32 Visualizer Firmware Development**

This phase focuses on creating the standalone firmware for the ESP32 microcontroller.

* **Task 2.1: Establish ESP32 Project with Arduino Core, FastLED, and OSC Server**  
  * **Description:** Create a new project directory (e.g., firmware/esp32\_visualizer/) outside the main Rust workspace. Configure the project to use PlatformIO with the espressif32 platform and the arduino framework. Add the FastLED library and a suitable Arduino OSC library (e.g., ArduinoOSC) as dependencies in platformio.ini. Implement a basic sketch that connects to a Wi-Fi network and initializes an OSC server that listens on a UDP port.  
  * **Files to Modify/Create:** firmware/esp32\_visualizer/platformio.ini, firmware/esp32\_visualizer/src/main.cpp.  
  * **Acceptance Criteria:** The ESP32 successfully connects to Wi-Fi. Sending a test OSC message from a separate tool (e.g., a Python script) results in a message being printed to the ESP32's serial monitor.  
* **Task 2.2: Implement the Configuration-Driven Hardware Model**  
  * **Description:** Create a board\_config.h header file within the ESP32 project. This file will use \#define directives to externalize all hardware-specific constants, such as LED\_PIN, NUM\_LEDS, LED\_TYPE, and COLOR\_ORDER, as specified in Section 2.4. Refactor the main C++ sketch to \#include "board\_config.h" and use these constants for initializing FastLED.  
  * **Files to Modify/Create:** firmware/esp32\_visualizer/src/board\_config.h, firmware/esp32\_visualizer/src/main.cpp.  
  * **Acceptance Criteria:** The firmware compiles and correctly drives the LED strip. Changing the LED\_PIN value in the config file and re-flashing the device successfully changes the output pin without any other code modifications.  
* **Task 2.3: Develop the Dual-Core FreeRTOS Task Architecture**  
  * **Description:** Refactor the standard Arduino setup() and loop() structure into two distinct FreeRTOS tasks. Use the xTaskCreatePinnedToCore function to explicitly assign tasks to CPU cores, a technique known to improve stability on the ESP32. The Network Task (pinned to Core 0\) will handle all Wi-Fi and OSC UDP packet reception, placing parsed commands into a FreeRTOS QueueHandle\_t. The Animation Task (pinned to Core 1\) will run a high-frequency loop, reading from the queue and updating the LEDs.  
  * **Files to Modify/Create:** firmware/esp32\_visualizer/src/main.cpp.  
  * **Acceptance Criteria:** The two tasks are created and run concurrently on separate cores, verifiable through logging. OSC messages received by the Network Task correctly trigger LED changes rendered by the Animation Task. The LED animation remains smooth even under simulated network load.  
* **Task 2.4: Implement MIDI-to-Visualization Logic and Effects**  
  * **Description:** Within the Animation Task, implement the core visualization logic. This includes creating a state machine (e.g., an array of structs) to track the state of all 128 MIDI notes (on/off, velocity). Map incoming OSC commands to changes in this state machine. Implement the visual effects described in the project plan, such as mapping velocity to brightness, creating a fade-out effect for noteOff events, and handling sustain pedal (CC 64\) messages to hold notes visually.  
  * **Files to Modify/Create:** firmware/esp32\_visualizer/src/main.cpp, firmware/esp32\_visualizer/src/effects.h (optional).  
  * **Acceptance Criteria:** Playing notes on a MIDI controller connected through the Android Hub (once complete) produces the correct, responsive visual effects on the LED strip. Specific effects like pitch bend and aftertouch are visibly represented.

### **Phase 3: Android Hub Application Development**

This phase involves building the central Android application that bridges the hardware controller and the network endpoints.

* **Task 3.1: Structure the Android Project for a Hybrid Native Architecture**  
  * **Description:** Create a new Android Studio project using Kotlin and Jetpack Compose. Configure the build.gradle file to execute the project's build\_android.sh script as part of the build process. This will compile the rtp\_midi\_lib Rust crate and automatically package the resulting .so shared library files for all target ABIs into the final APK. Set up the JNI boilerplate in a Kotlin class to load the native library (System.loadLibrary("rtp\_midi\_lib")).  
  * **Files to Modify/Create:** New Android project files: app/build.gradle.kts, app/src/main/java/.../MainActivity.kt.  
  * **Acceptance Criteria:** A simple "Hello World" external fun declared in Kotlin and implemented in platform/src/ffi.rs can be successfully called from the Android app, demonstrating that the JNI bridge is correctly configured.  
* **Task 3.2: Implement Low-Latency USB MIDI Input via the AMidi NDK API**  
  * **Description:** This is a critical, high-risk task. In the Kotlin layer, use the android.media.midi.MidiManager to enumerate and request permission for the connected Maschine controller. Once permission is granted, pass the MidiDevice object or its native handle to the Rust/NDK layer via a new JNI function. In the Rust code, use JNI bindings to interact with the C-based AMidi functions to open an input port and read MIDI data directly from the device. This bypasses the higher-latency Java MIDI callback system for the real-time data path.  
  * **Files to Modify/Create:** platform/src/ffi.rs, rtp\_midi\_lib/src/lib.rs, new Kotlin MidiService.kt.  
  * **Acceptance Criteria:** MIDI events generated by the Maschine controller are successfully received and logged from within the Rust core with minimal and consistent latency.  
* **Task 3.3: Integrate RTP-MIDI and OSC Senders into the Rust Core**  
  * **Description:** In the main service loop located in rtp\_midi\_lib/src/lib.rs, plumb the incoming MIDI data stream from the AMidi input (Task 3.2) to two separate processing paths. The first path will forward the raw MIDI commands to the completed RTP-MIDI session (Task 1.1). The second path will translate the MIDI commands into OSC messages and send them to the new OSC sender (Task 1.2).  
  * **Files to Modify:** rtp\_midi\_lib/src/lib.rs.  
  * **Acceptance Criteria:** A single MIDI Note On event from the Maschine controller results in both a corresponding RTP-MIDI packet being sent to the DAW and an OSC /noteOn message being sent to the ESP32.  
* **Task 3.4: Develop the Android Foreground Service and a Minimal UI**  
  * **Description:** Create a Kotlin ForegroundService that uses the JNI bridge to call the create\_service, start\_service, and stop\_service functions in the Rust core. Ensure the service is correctly declared with type connectedDevice in the AndroidManifest.xml. Implement a minimal UI using Jetpack Compose that provides buttons to start/stop the service and displays the status and discovered IP addresses of the DAW and ESP32.  
  * **Files to Modify/Create:** New MidiHubService.kt, MainActivity.kt, AndroidManifest.xml.  
  * **Acceptance Criteria:** The MIDI routing service runs reliably in the background, even when the app is not in the foreground. The UI correctly reflects the connection status and can control the service lifecycle.

### **Phase 4: Integration, Testing, and Refinement**

The final phase focuses on end-to-end testing, code quality, and documentation.

* **Task 4.1: Develop and Automate End-to-End Integration Tests**  
  * **Description:** Expand the integration\_tests crate to validate the new architecture. Create new test modules that mock the network endpoints. One test will act as an RTP-MIDI participant, receiving packets from the service and asserting their contents. Another test will open a UDP socket to act as a mock ESP32, receiving OSC packets and verifying their format and data. These new test suites should be added to the ci.yml workflow.  
  * **Files to Modify/Create:** integration\_tests/tests/rtp\_midi\_e2e.rs, integration\_tests/tests/osc\_e2e.rs, .github/workflows/ci.yml.  
  * **Acceptance Criteria:** The CI pipeline automatically runs tests that validate the entire data flow from a simulated MIDI input through the Rust core to both the RTP-MIDI and OSC network outputs.  
* **Task 4.2: Perform Code Quality Sweep and CI/CD Enhancements**  
  * **Description:** Address the remaining code quality TODOs identified in the README.md, such as enforcing the zero-warning policy by adding \#\!\[deny(warnings)\] to the top of each crate's lib.rs or main.rs file. Fix all resulting warnings (e.g., unused variables, unreachable code). Implement the proposed "cargo fix" CI job to automate code style suggestions.  
  * **Files to Modify:** Various .rs files across the workspace, .github/workflows/ci.yml.  
  * **Acceptance Criteria:** The command cargo clippy \--all-targets \-- \-D warnings passes cleanly across the entire workspace. A new CI job is created that can automatically create pull requests with cargo fix suggestions.  
* **Task 4.3: Update Project Documentation**  
  * **Description:** Thoroughly revise the project's documentation to reflect the new, consolidated architecture. The main README.md and the documents in docs/architecture/ must be updated to describe the "Maschine to LED" system as the primary use case. Documentation related to now-deprecated components (e.g., the WebRTC signaling server, the Qt UI, the multiple server binaries) should be removed or moved to a separate "archive" directory to avoid confusion.  
  * **Files to Modify:** README.md, docs/architecture/rtp-midi-architecture.md, and other related documents.  
  * **Acceptance Criteria:** The project's documentation accurately and clearly describes the current architecture, setup, and usage of the rtp-midi system in its new form.

## **Section 4: Strategic Recommendations**

To ensure the long-term success and maintainability of the rtp-midi project as it evolves, the following strategic recommendations should be considered.

### **4.1 Codebase Consolidation and Deprecation Strategy**

The implementation of the "Maschine MIDI na ESP32 LED" plan should be treated as a strategic pivot for the project. The current codebase contains multiple server binaries (audio\_server, signaling\_server) and several frontend implementations (qt\_ui, ui-frontend, frontend) that are not aligned with the new target architecture. It is strongly recommended to formally deprecate these components. This action will focus development effort, simplify the repository structure, reduce maintenance overhead, and provide clarity for new contributors. The rtp\_midi\_node binary, when compiled for the Android target, will become the primary application artifact. A clear deprecation notice should be added to the main README.md, explaining the project's new focus and directing users to the Android Hub application.

### **4.2 Advanced Testing and Profiling**

The low-latency requirements of a real-time musical instrument necessitate a more rigorous testing and profiling strategy than is currently in place.

* **FFI/JNI Boundary Testing:** The FFI layer is a common source of bugs related to memory management and ABI compatibility. It is recommended to use a tool like cargo-c-test to create C-based unit tests that directly call the extern "C" functions in platform/src/ffi.rs. This will help catch memory safety issues and ensure the bridge remains stable.  
* **Latency Measurement:** To validate and optimize performance, a "loopback" test should be created. This test would involve the Android Hub sending a specific OSC message to the ESP32, which would be programmed to immediately send a response message back. By measuring the round-trip time within the Android application, developers can obtain a concrete metric for the real-world latency of the Wi-Fi and OSC protocol stack, allowing for targeted optimization.  
* **Android Native Profiling:** Developers should make extensive use of the Android Studio Profiler. Specifically, the CPU profiler should be used to monitor the native Rust/C++ thread to ensure it runs with the correct high priority and is not being preempted by other processes. The memory profiler can help verify that the native code is not performing unexpected allocations that could trigger the Java Garbage Collector, a common source of audio stutter and latency.

### **4.3 Future Architectural Scalability**

While implementing the current plan, architectural decisions should be made with future extensibility in mind.

* **ESP32 Visualization Engine:** The visualization logic in the ESP32 firmware should be designed around a trait-based or object-oriented pattern. For instance, a C++ abstract base class VisualizerEffect could be defined with virtual methods like onNoteOn(), onNoteOff(), and renderFrame(). Different visual effects (e.g., "PianoScroll", "SpectrumAnalyzer", "Rainfall") could then be implemented as separate classes inheriting from this base class. This would allow new effects to be added without modifying the core animation loop, making the firmware highly extensible.  
* **Protocol Abstraction:** The DataStreamNetSender and DataStreamNetReceiver traits in the Rust core are an excellent pattern for abstracting output and input streams. This pattern should be preserved and leveraged for future features. For example, if support for MIDI 2.0 over RTP were desired, a new RtpMidi2Session could be created. As long as it implements the relevant sender/receiver traits, it could be integrated into the existing architecture with minimal friction, demonstrating the power of the project's modular design.

#### **Works cited**

1\. Midi \- NDK \- Android Developers, https://developer.android.com/ndk/reference/group/midi 2\. rtpmidi \- crates.io: Rust Package Registry, https://crates.io/crates/rtpmidi/dependencies 3\. rtpmidi \- Lib.rs, https://lib.rs/crates/rtpmidi 4\. rosc — Rust audio library // Lib.rs, https://lib.rs/crates/rosc 5\. rosc \- Rust \- Docs.rs, https://docs.rs/rosc 6\. mDNS for ESP32 \- RNT Lab, https://rntlab.com/question/mdns-for-esp32/ 7\. Help understanding mDNS on ESP32 \- Programming \- Arduino Forum, https://forum.arduino.cc/t/help-understanding-mdns-on-esp32/1026655 8\. AppleMIDI \- Arduino Documentation, https://docs.arduino.cc/libraries/applemidi/ 9\. MIDI keyboards, interfaces, latency with android \- Other Instruments \- Basschat, https://www.basschat.co.uk/topic/488702-midi-keyboards-interfaces-latency-with-android/ 10\. Android low latency / high performance audio in 2023 : r/androiddev \- Reddit, https://www.reddit.com/r/androiddev/comments/16rpp9z/android\_low\_latency\_high\_performance\_audio\_in\_2023/ 11\. Oboe: A C++ library for low latency audio in Android \- Hacker News, https://news.ycombinator.com/item?id=18198177 12\. Unreliable results from ESP32 and FastLED (but probably my code) \- Reddit, https://www.reddit.com/r/FastLED/comments/10egfcl/unreliable\_results\_from\_esp32\_and\_fastled\_but/ 13\. FastLED Library, https://fastled.io/docs/ 14\. ws2812-esp32-rmt-driver \- crates.io: Rust Package Registry, https://crates.io/crates/ws2812-esp32-rmt-driver 15\. ws2812\_esp32\_rmt\_driver \- Rust \- Docs.rs, https://docs.rs/ws2812-esp32-rmt-driver 16\. Easiest way to get into embedded with an esp32 : r/rust \- Reddit, https://www.reddit.com/r/rust/comments/19d6qlv/easiest\_way\_to\_get\_into\_embedded\_with\_an\_esp32/ 17\. Low Latency Rust \- Gabriel Dalboozi \- YouTube, https://www.youtube.com/watch?v=N9RzbcWsqiw