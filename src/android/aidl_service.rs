// src/android/aidl_service.rs

//! AIDL service implementation for Android IPC (Rust side)

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use log::{info, error};
use tokio::runtime::{Runtime, Handle};
use rsbinder::{Interface, Result as BinderResult, Strong, StatusCode};

use crate::{Config, run_service_loop, wled_control};

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/com/example/rtpmidi/IMidiWledService.rs"));

// Dummy trait for non-Android targets to prevent compilation errors
#[cfg(not(target_os = "android"))]
pub trait IMidiWledService {
    // Define a minimal set of methods or an empty trait if no common interface is needed.
    // Or, simply make the code that depends on IMidiWledService conditional as well.
    fn startListener(&self) -> BinderResult<bool> {
        // Dummy implementation for non-Android
        info!("Dummy FFI: startListener called (non-Android)");
        Ok(false)
    }
    fn stopListener(&self) -> BinderResult<()> {
        // Dummy implementation for non-Android
        info!("Dummy FFI: stopListener called (non-Android)");
        Ok(())
    }
    fn setWledPreset(&self, preset_id: i32) -> BinderResult<()> {
        // Dummy implementation for non-Android
        info!("Dummy FFI: setWledPreset called (non-Android) with id {}", preset_id);
        Ok(())
    }
    fn getStatus(&self) -> BinderResult<String> {
        // Dummy implementation for non-Android
        info!("Dummy FFI: getStatus called (non-Android)");
        Ok("Not Running (Non-Android)".to_string())
    }
    fn isRunning(&self) -> BinderResult<bool> {
        // Dummy implementation for non-Android
        info!("Dummy FFI: isRunning called (non-Android)");
        Ok(false)
    }
}

#[cfg(target_os = "android")]
pub mod aidl_impl;

#[cfg(not(target_os = "android"))]
pub mod aidl_impl {
    // Dummy module for non-Android targets
}

// Název služby, pod kterým bude registrována v Android Service Manageru.
pub const SERVICE_NAME: &str = "com.example.rtpmidi.MidiWledService";

/// Vnitřní stav služby, sdílený mezi vlákny.
struct ServiceState {
    config: Config,
    running_flag: Arc<AtomicBool>,
    status_message: String,
    // Handle na pracovní vlákno, abychom na něj mohli počkat.
    worker_thread: Option<thread::JoinHandle<()>>,
    // Tokio runtime pro asynchronní operace (např. volání WLED API).
    tokio_rt_handle: Handle, // Store only the handle
}

impl ServiceState {
    fn new(config: Config, tokio_rt_handle: Handle) -> Self {
        Self {
            config,
            running_flag: Arc::new(AtomicBool::new(false)),
            status_message: "Stopped".to_string(),
            worker_thread: None,
            tokio_rt_handle,
        }
    }
}

/// Hlavní struktura, která implementuje AIDL rozhraní.
pub struct MidiWledService {
    state: Arc<Mutex<ServiceState>>,
}

impl MidiWledService {
    pub fn new(config: Config, tokio_rt_handle: Handle) -> Self {
        Self {
            state: Arc::new(Mutex::new(ServiceState::new(config, tokio_rt_handle))),
        }
    }
}

// Implementace pro `rsbinder::Interface`. Je nutná pro každou službu.
impl Interface for MidiWledService {}

// Implementace samotného AIDL rozhraní `IMidiWledService`.
// Názvy metod musí odpovídat definici v `.aidl` souboru.
impl IMidiWledService for MidiWledService {
    fn startListener(&self) -> BinderResult<bool> {
        info!("AIDL call: startListener()");
        let mut state = self.state.lock().unwrap();

        if state.running_flag.load(Ordering::SeqCst) {
            info!("Listener is already running.");
            return Ok(true);
        }

        info!("Starting listener via AIDL...");
        
        let config_clone = state.config.clone();
        let running_clone = state.running_flag.clone();
        let rt_handle_clone = state.tokio_rt_handle.clone(); // Clone the handle

        // Nastavíme flag před spuštěním vlákna.
        running_clone.store(true, Ordering::SeqCst);

        let thread_handle = thread::spawn(move || {
            // Tato funkce je z `lib.rs` a obsahuje hlavní logiku.
            run_service_loop(config_clone, running_clone);
        });

        state.worker_thread = Some(thread_handle);
        state.status_message = "Running".to_string();
        
        info!("Listener started successfully.");
        Ok(true)
    }

    fn stopListener(&self) -> BinderResult<()> {
        info!("AIDL call: stopListener()");
        let mut state = self.state.lock().unwrap();

        if !state.running_flag.load(Ordering::SeqCst) {
            info!("Listener is not running.");
            return Ok(());
        }

        info!("Stopping listener via AIDL...");
        
        // Signalizujeme pracovnímu vláknu, aby se ukončilo.
        state.running_flag.store(false, Ordering::SeqCst);

        if let Some(handle) = state.worker_thread.take() {
            // Počkáme, až se vlákno korektně ukončí.
            handle.join().expect("Failed to join worker thread.");
        }
        
        state.status_message = "Stopped".to_string();
        info!("Listener stopped successfully.");
        Ok(())
    }

    fn setWledPreset(&self, preset_id: i32) -> BinderResult<()> {
        info!("AIDL call: setWledPreset(id={})", preset_id);
        let state = self.state.lock().unwrap();
        
        let wled_ip = state.config.wled_ip.clone();
        let rt_handle = state.tokio_rt_handle.clone(); // Get handle instead of Arc<Runtime>

        // Spawn the async task without blocking the binder thread
        rt_handle.spawn(async move {
            if let Err(e) = wled_control::set_wled_preset(&wled_ip, preset_id).await {
                error!("Failed to set WLED preset from AIDL: {}", e);
            }
        });
        
        Ok(())
    }

    fn getStatus(&self) -> BinderResult<String> {
        info!("AIDL call: getStatus()");
        let state = self.state.lock().unwrap();
        Ok(state.status_message.clone())
    }

    fn isRunning(&self) -> BinderResult<bool> {
        info!("AIDL call: isRunning()");
        let state = self.state.lock().unwrap();
        Ok(state.running_flag.load(Ordering::SeqCst))
    }
}

/// Funkce pro vytvoření a registraci služby.
pub fn register_service(config_path: &str, rt_handle: Handle) {
    info!("Attempting to register Rust AIDL service...");

    let config = match Config::load_from_file(config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("FATAL: Could not load config from '{}'. Service cannot start. Error: {}", config_path, e);
            return; // Stop execution
        }
    };
    
    // Vytvoření instance naší služby.
    let service = MidiWledService::new(config, rt_handle);
    let binder: Strong<dyn IMidiWledService> = BnMidiWledService::new_binder(service, Default::default());

    // Registrace služby u Android Service Manageru.
    match rsbinder::hub::add_service(SERVICE_NAME, binder.as_binder()) {
        Ok(_) => {
            info!("Successfully registered '{}'. Joining thread pool.", SERVICE_NAME);
            // Udržíme vlákno naživu, aby služba mohla přijímat požadavky.
            rsbinder::ProcessState::join_thread_pool();
        }
        Err(e) => {
            error!("Failed to register service: {:?}", e);
        }
    }
}
