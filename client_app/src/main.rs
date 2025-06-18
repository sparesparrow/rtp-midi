use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use log::{info, error};
use rtp_midi_lib::{Config, run_service_loop};
use tokio::runtime::Runtime;

/// Main entry point for the desktop application.
/// This loads the configuration and runs the service loop until interrupted.
fn main() {
    // Initialize logging from the RUST_LOG environment variable, defaulting to "info"
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Load the application configuration from `config.toml`
    let config = match Config::load_from_file("config.toml") {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load config.toml: {}", e);
            std::process::exit(1);
        }
    };
    info!("Configuration loaded successfully: {:?}", config);

    // Create a new Tokio runtime for the main application
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Create an atomically-shared boolean flag to signal the service to stop.
    // This allows the Ctrl+C handler to communicate with the main service loop.
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up a handler to listen for Ctrl+C.
    // When the signal is received, it sets the `running` flag to false.
    ctrlc::set_handler(move || {
        info!("Ctrl+C signal received, initiating shutdown...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    info!("Service starting. Press Ctrl+C to stop.");

    // Pass the runtime handle to the service loop
    rt.block_on(async {
        run_service_loop(config, running).await;
    });

    info!("Service has shut down gracefully.");
}