use tokio::sync::watch;
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

    // Set up a unified shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    // Set up a handler to listen for Ctrl+C and send shutdown signal
    ctrlc::set_handler(move || {
        info!("Ctrl+C signal received, initiating shutdown...");
        let _ = shutdown_tx.send(true);
    }).expect("Error setting Ctrl-C handler");

    info!("Service starting. Press Ctrl+C to stop.");

    // Pass the runtime handle to the service loop
    rt.block_on(async {
        run_service_loop(config, shutdown_rx).await;
    });

    info!("Service has shut down gracefully.");
}