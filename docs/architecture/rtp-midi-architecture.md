# Architektura projektu rtp-midi

Tento dokument shrnuje modulární a škálovatelnou architekturu projektu **rtp-midi** postavenou na idiomatických principech Rustu, jako jsou workspaces, feature-gated moduly a oddělení core logiky od platformně-specifického kódu.

---

## 1. Přehledná struktura workspace

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
        B2("<b>service-bus</b><br/><i>Async services & mpsc channels</i>")
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

---

## 2. Modulární crates a jejich role

```mermaid
flowchart TD
    A1["Cargo.toml (root)"]
    A2["crates/"]
    A3["bin/"]
    A4[".github/workflows/"]
    B1["core\nPlatform-agnostic logic\nno_std, traits, protocols"]
    C1["hal-pc\nPC-specific impl."]
    C2["hal-android\nAndroid impl."]
    C3["hal-esp32\nESP32 impl."]
    D1["service-bus\nAsync comms, mpsc"]
    E1["ui-frontend\nWASM/Tauri, WebSocket"]
    F1["rtp-midi-node.rs\nRole/platform detection"]
    G1["ci.yml"]
    G2["release.yml"]
    G3["audit.yml"]
    A1 --> A2
    A1 --> A3
    A1 --> A4
    A2 --> B1
    A2 --> D1
    A2 --> C1
    A2 --> C2
    A2 --> C3
    A2 --> E1
    A3 --> F1
    A4 --> G1
    A4 --> G2
    A4 --> G3
    B1 -.trait impls.-> C1
    B1 -.trait impls.-> C2
    B1 -.trait impls.-> C3
    F1 --WebSocket--> E1
```

- **core**: Platformně nezávislá logika, traity, protokoly, no_std.
- **hal-***: Platform-specific implementace (PC, Android, ESP32), aktivované feature flagy.
- **service-bus**: Asynchronní message passing (např. tokio mpsc).
- **ui-frontend**: Oddělené UI, např. WASM/Tauri, komunikace přes WebSocket.
- **rtp-midi-node.rs**: Jediný binární entrypoint, autodetekce role/platformy.

---

## 3. Feature flagy a build-time selekce

```mermaid
flowchart TD
    FF1["Cargo.toml (root)"]
    FF2["[features]\nhal_pc, hal_esp32, hal_android, ui"]
    FF3["cargo build --features ..."]
    FF1 --> FF2
    FF2 --> FF3
    FF3 --> HAL1["hal-pc"]
    FF3 --> HAL2["hal-esp32"]
    FF3 --> HAL3["hal-android"]
    FF3 --> UI1["ui-frontend"]
```

- V root `Cargo.toml` jsou definovány feature flagy pro jednotlivé platformy a UI.
- Build pouze relevantních částí pro danou platformu.

---

## 4. Autodetekce role a platformy za běhu

```mermaid
flowchart TD
    BIN["rtp-midi-node.rs"]
    DETECT["Detect HW/role at runtime"]
    ROLE1["Server"]
    ROLE2["Client"]
    ROLE3["UI-host"]
    BIN --> DETECT
    DETECT --> ROLE1
    DETECT --> ROLE2
    DETECT --> ROLE3
```

- Jediný binární soubor detekuje za běhu, na jakém HW běží a jakou má plnit roli.

---

## 5. Oddělené UI a WebSocket API

```mermaid
flowchart TD
    UI1["ui-frontend (WASM/Tauri)"]
    WS["WebSocket API"]
    NODE["rtp-midi-node"]
    UI1 -- WebSocket --> WS
    WS -- WebSocket --> NODE
```

- UI je samostatný crate, který lze zkompilovat do WASM a připojit se k jakémukoliv rtp-midi-node přes WebSocket.
- UI je zcela oddělené od backendové logiky.

---

## 6. CI/CD workflowy

```mermaid
flowchart TD
    WF[".github/workflows/"]
    CI["ci.yml"]
    REL["release.yml"]
    AUD["audit.yml"]
    WF --> CI
    WF --> REL
    WF --> AUD
```

- Automatizace buildů, testů, auditů.

---

## Shrnutí klíčových principů

- **Workspace (Cargo.toml v rootu):** Spravuje všechny crates jako jeden celek, zjednodušuje build a správu závislostí. Definuje feature flagy (hal_pc, hal_esp32, ui).
- **core crate:** Obsahuje čistou, platformně nezávislou (no_std) logiku. Lze ji použít na PC i na mikrokontroleru.
- **hal-* crates:** Každý crate implementuje společné traity z core pro specifickou platformu. Aktivuje se pouze s příslušným feature flagem.
- **Jediná binárka:** Místo mnoha různých programů existuje jen jeden entrypoint, který při startu detekuje roli a platformu.
- **Oddělené UI:** Uživatelské rozhraní je samostatný crate, který lze zkompilovat do WASM a připojit se k backendu přes WebSocket.
- **Robustní, snadno testovatelná a rozšiřitelná architektura.** 