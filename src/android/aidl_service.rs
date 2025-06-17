//! AIDL service implementation stub for Android IPC (Rust side)
//! Uses libbinder_rs and the generated AIDL trait.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use log::{info, error};
use tokio::runtime::Runtime;

// Assuming the AIDL generated code will be available in the build process.
// The path might need adjustment based on your build system (e.g., soong, buck2).
// For this example, we assume `binder_rs` has generated this crate.
// use com_example_rustmidiservice::aidl::com::example::rustmidiservice::IMidiWledService::{BnMidiWledService, IMidiWledService};
// use binder::{Interface, BinderFeatures, Result as BinderResult, StatusCode};

use crate::{Config, wled_control};
use crate::{start_audio_input, compute_fft_magnitudes, map_audio_to_leds, create_ddp_sender, send_ddp_frame};

/// Holds the state of our service.
struct ServiceState {
    config: Config,
    is_running: AtomicBool,
    status_message: String,
    // Handle to the processing thread, so we can wait for it to finish.
    worker_thread: Option<thread::JoinHandle<()>>,
}

impl ServiceState {
    fn new(config: Config) -> Self {
        Self {
            config,
            is_running: AtomicBool::new(false),
            status_message: "Stopped".to_string(),
            worker_thread: None,
        }
    }
}

/// The actual implementation of the AIDL service.
pub struct MidiWledService {
    state: Arc<Mutex<ServiceState>>,
}

impl MidiWledService {
    fn new(state: Arc<Mutex<ServiceState>>) -> Self {
        Self { state }
    }
}

/*
// This block requires the generated AIDL bindings. It won't compile without them.
// Uncomment this when you have the AIDL build process set up.

impl Interface for MidiWledService {}

impl IMidiWledService for MidiWledService {
    fn startListener(&self) -> BinderResult<bool> {
        let mut state = self.state.lock().unwrap();
        if state.is_running.load(Ordering::SeqCst) {
            info!("Listener is already running.");
            return Ok(true);
        }

        info!("Starting listener...");
        let state_clone = Arc::clone(&self.state);
        let cancellation_token = Arc::new(AtomicBool::new(false));

        let thread_handle = thread::spawn(move || {
            let core_state = state_clone.lock().unwrap();
            let config = core_state.config.clone();
            drop(core_state); // Release lock before long-running task

            run_core_logic(config, cancellation_token);
        });

        state.worker_thread = Some(thread_handle);
        state.is_running.store(true, Ordering::SeqCst);
        state.status_message = "Running".to_string();
        info!("Listener started successfully.");

        Ok(true)
    }

    fn stopListener(&self) -> BinderResult<()> {
        let mut state = self.state.lock().unwrap();
        if !state.is_running.load(Ordering::SeqCst) {
            info!("Listener is not running.");
            return Ok(());
        }

        info!("Stopping listener...");
        if let Some(cancellation_token) = state.is_running.compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed).ok() {
            // How to stop the thread? We need a cancellation token.
            // For now, we just wait for it to join, but the thread needs to check a flag.
             if let Some(handle) = state.worker_thread.take() {
                // Here we would signal the thread to stop.
                // handle.join().expect("Failed to join worker thread.");
             }
        }
        
        state.status_message = "Stopped".to_string();
        info!("Listener stopped.");
        Ok(())
    }

    fn setWledPreset(&self, preset_id: i32) -> BinderResult<()> {
        info!("Setting WLED preset to {}", preset_id);
        let state = self.state.lock().unwrap();
        let wled_ip = state.config.wled_ip.clone();

        // We need an async runtime to call the wled_control functions.
        // Spawning a new runtime for each call is inefficient but simple.
        // A better approach would be a shared runtime managed by the service.
        let rt = Runtime::new().map_err(|e| {
            error!("Failed to create Tokio runtime: {}", e);
            StatusCode::UNEXPECTED_NULL
        })?;
        
        rt.block_on(async {
            if let Err(e) = wled_control::set_wled_preset(&wled_ip, preset_id).await {
                error!("Failed to set WLED preset: {}", e);
                // We can't return a specific error here easily with BinderResult<()>,
                // but we log it.
            }
        });
        
        Ok(())
    }

    fn getStatus(&self) -> BinderResult<String> {
        let state = self.state.lock().unwrap();
        Ok(state.status_message.clone())
    }
}
*/

/// Main logic loop for audio processing and DDP output.
/// This is adapted from the test loop in `src/main.rs`.
fn run_core_logic(config: Config, running: Arc<AtomicBool>) {
    info!("Core logic thread started.");

    // Setup channels
    let (audio_tx, audio_rx) = crossbeam_channel::bounded(8);
    let (fft_tx, fft_rx) = crossbeam_channel::bounded(8);
    let (led_tx, led_rx) = crossbeam_channel::bounded(8);

    // --- Audio Input Thread ---
    let audio_config = config.clone();
    let running_audio = running.clone();
    let audio_handle = thread::spawn(move || {
        match start_audio_input(audio_config.audio_device.as_deref(), audio_tx) {
            Ok(stream) => {
                stream.play().expect("Failed to start audio stream");
                info!("Audio input stream started.");
                while running_audio.load(Ordering::Relaxed) {
                    thread::sleep(std::time::Duration::from_millis(100));
                }
                info!("Audio input stream stopped.");
            }
            Err(e) => error!("Failed to start audio input: {}", e),
        }
    });

    // --- Audio Analysis Thread ---
    let running_analysis = running.clone();
    let analysis_handle = thread::spawn(move || {
        let mut prev = Vec::new();
        let smoothing = 0.7; // Could be part of config
        while running_analysis.load(Ordering::Relaxed) {
             if let Ok(buffer) = audio_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                let mags = compute_fft_magnitudes(&buffer, &mut prev, smoothing);
                if fft_tx.send(mags).is_err() {
                    break; // Channel closed
                }
            }
        }
        info!("Audio analysis thread stopped.");
    });
    
    // --- Light Mapping Thread ---
    let led_count_mapping = config.led_count;
    let running_mapping = running.clone();
    let mapping_handle = thread::spawn(move || {
        while running_mapping.load(Ordering::Relaxed) {
            if let Ok(mags) = fft_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                let leds = map_audio_to_leds(&mags, led_count_mapping);
                if led_tx.send(leds).is_err() {
                    break; // Channel closed
                }
            }
        }
        info!("Light mapping thread stopped.");
    });

    // --- DDP Output Thread ---
    let ddp_config = config.clone();
    let running_ddp = running.clone();
    let ddp_handle = thread::spawn(move || {
        let rgbw = ddp_config.color_format.as_deref().unwrap_or("RGB").eq_ignore_ascii_case("RGBW");
        let mut sender = match create_ddp_sender(&ddp_config.wled_ip, ddp_config.ddp_port.unwrap_or(4048), ddp_config.led_count, rgbw) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create DDP sender: {}", e);
                return;
            }
        };
        info!("DDP sender created for {}", ddp_config.wled_ip);

        while running_ddp.load(Ordering::Relaxed) {
            if let Ok(leds) = led_rx.recv_timeout(std::time::Duration::from_millis(100))) {
                if let Err(e) = send_ddp_frame(&mut sender, &leds) {
                    error!("Failed to send DDP frame: {}", e);
                }
            }
        }
        info!("DDP output thread stopped.");
    });

    // Wait for all threads to finish their work
    audio_handle.join().expect("Audio input thread panicked");
    analysis_handle.join().expect("Audio analysis thread panicked");
    mapping_handle.join().expect("Light mapping thread panicked");
    ddp_handle.join().expect("DDP output thread panicked");
    
    info!("Core logic has shut down.");
}

/// Registers the service with the Android Service Manager.
/// This should be called from your JNI entry point (e.g., `JNI_OnLoad`).
pub fn register_service() {
    /*
    // This part is also dependent on the AIDL generated code.
    info!("Attempting to register Rust AIDL service...");

    // 1. Load config
    let config = Config::load_from_file("config.toml").expect("Failed to load config for service");
    
    // 2. Create service state
    let state = Arc::new(Mutex::new(ServiceState::new(config)));
    
    // 3. Create service instance
    let service = MidiWledService::new(state);
    let binder = BnMidiWledService::new_binder(service, BinderFeatures::default());

    // 4. Register with ServiceManager
    match binder::add_service("midi_wled_service", binder.as_binder()) {
        Ok(_) => {
            info!("Successfully registered 'midi_wled_service'. Joining thread pool.");
            // Keep the service alive
            binder::ProcessState::join_thread_pool();
        }
        Err(e) => {
            error!("Failed to register service: {:?}", e);
        }
    }
    */
}
