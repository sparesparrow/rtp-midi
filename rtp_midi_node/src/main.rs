#[cfg(feature = "ctrlc")]
use ctrlc;
use log::info;
use std::env;
use tokio::sync::watch;

fn main() {
    let args: Vec<String> = env::args().collect();
    let role = args
        .iter()
        .position(|a| a == "--role")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    match role {
        Some("server") => {
            println!("[rtp-midi-node] Spouštím v režimu SERVER");
            let config = rtp_midi_lib::Config::load_from_file("config.toml")
                .expect("config.toml načtení selhalo");
            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            #[cfg(feature = "ctrlc")]
            ctrlc::set_handler(move || {
                info!("Ctrl+C signal received, initiating shutdown...");
                let _ = shutdown_tx.send(true);
            })
            .expect("Error setting Ctrl-C handler");
            #[cfg(not(feature = "ctrlc"))]
            println!("Ctrl+C handler not available; shutdown must be triggered manually.");
            let service = tokio::spawn(async move {
                rtp_midi_lib::run_service_loop(config, shutdown_rx).await;
            });
        }
        Some("client") => {
            println!("[rtp-midi-node] Spouštím v režimu CLIENT");
            println!("Spusťte klientskou aplikaci přímo: cargo run -p client_app");
        }
        Some("ui-host") => {
            println!("[rtp-midi-node] Spouštím v režimu UI-HOST");
            // Spustit jednoduchý HTTP server pro WASM UI
            // (vyžaduje simple-server nebo warp v [dependencies])
            // TODO: Přidejte lepší webserver nebo integraci s Tauri
            simple_server::Server::new(|_req, mut res| {
                let path = _req.uri().path();
                let file = match path {
                    "/" => "ui-frontend/index.html",
                    p if p.starts_with("/pkg/") => &p[1..],
                    _ => "ui-frontend/index.html",
                };
                match std::fs::read(file) {
                    Ok(body) => Ok(res.body(body)?),
                    Err(_) => Ok(res.status(404).body(b"Not found".to_vec())?),
                }
            })
            .listen("127.0.0.1", "8088");
            println!("UI dostupné na http://127.0.0.1:8088/");
        }
        _ => {
            eprintln!("Použití: rtp-midi-node --role [server|client|ui-host]");
            std::process::exit(1);
        }
    }

    // TODO: Pro embedded/ESP32 buildy lze autodetekovat platformu přes feature flagy nebo env proměnné
}

// POZOR: Pokud build selže kvůli chybějící závislosti simple-server, přidejte ji do [dependencies] v Cargo.toml root crate.
