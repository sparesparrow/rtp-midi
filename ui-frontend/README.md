# ui-frontend (WASM/Yew)

Toto je Rust-based UI pro projekt rtp-midi, postavené na frameworku [Yew](https://yew.rs/) a kompilované do WebAssembly (WASM).

## Build a spuštění

1. Nainstalujte [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/):
   ```bash
   cargo install wasm-pack
   ```
2. Sestavte UI:
   ```bash
   wasm-pack build --target web
   ```
3. Výstup najdete v `pkg/`. Pro lokální testování použijte např. [simple-http-server](https://github.com/TheWaWaR/simple-http-server):
   ```bash
   simple-http-server ./ --index
   ```
4. Otevřete `index.html` a načtěte WASM bundle.

## Architektura
- UI je oddělený crate, komunikace s backendem probíhá přes WebSocket API.
- Pro rozšíření UI přidejte komponenty do `src/lib.rs` a napojte na WebSocket.
- Propojení s backendem (rtp-midi-node) je možné přes JSON zprávy (viz dokumentace backendu).

## Příklad napojení na backend
```rust
use web_sys::{WebSocket, MessageEvent};
let ws = WebSocket::new("ws://localhost:8080/ws").unwrap();
// Přidejte event listenery a logiku podle potřeby
```

## TODO
- Přidat reálné UI komponenty (ovládání, monitoring, mapping editor...)
- Přidat příklady komunikace s backendem
- Přidat build skript pro automatizaci 