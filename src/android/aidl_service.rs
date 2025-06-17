// src/android/aidl_service.rs

//! AIDL service implementation for Android IPC (Rust side)

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use log::{info, error};
use tokio::runtime::Runtime;
use rsbinder::{Interface, BinderFeatures, Result as BinderResult, Strong, StatusCode};

use crate::{Config, run_service_loop, wled_control};

pub mod service;
pub mod types;

// Zahrnutí vygenerovaného kódu z AIDL.
// Cargo automaticky najde tento soubor v `OUT_DIR`.
include!(concat!(env!("OUT_DIR"), "/com/example/rtpmidi/IMidiWledService.rs"));

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
    tokio_rt: Arc<Runtime>,
}

impl ServiceState {
    fn new(config: Config) -> Self {
        Self {
            config,
            running_flag: Arc::new(AtomicBool::new(false)),
            status_message: "Stopped".to_string(),
            worker_thread: None,
            tokio_rt: Arc::new(Runtime::new().expect("Failed to create Tokio runtime for service")),
        }
    }
}

/// Hlavní struktura, která implementuje AIDL rozhraní.
pub struct MidiWledService {
    state: Arc<Mutex<ServiceState>>,
}

impl MidiWledService {
    pub fn new(config: Config) -> Self {
        Self {
            state: Arc::new(Mutex::new(ServiceState::new(config))),
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
        let rt = state.tokio_rt.clone();

        // Blokujeme na asynchronní operaci.
        // Je to v pořádku, protože `setWledPreset` je rychlá síťová operace.
        rt.block_on(async {
            if let Err(e) = wled_control::set_wled_preset(&wled_ip, preset_id).await {
                error!("Failed to set WLED preset from AIDL: {}", e);
                // Zde bychom mohli vrátit specifickou chybu, pokud by to AIDL podporovalo lépe.
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
pub fn register_service() {
    info!("Attempting to register Rust AIDL service...");

    // POZNÁMKA: Cesta ke konfiguračnímu souboru je zde pevně daná.
    // V reálné aplikaci by měla být cesta předána z Android (Java/Kotlin) strany,
    // např. při startu služby.
    let config_path = "/data/data/com.example.rtpmidi/files/config.toml";
    let config = match Config::load_from_file(config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("FATAL: Could not load config from '{}'. Service cannot start. Error: {}", config_path, e);
            // Zde by aplikace měla selhat, protože bez konfigurace nemůže fungovat.
            // Pro jednoduchost jen zalogujeme a pokračujeme, ale služba nebude funkční.
            // Vytvoříme prázdnou konfiguraci, aby se kód zkompiloval.
            Config {
                wled_ip: "0.0.0.0".to_string(),
                ddp_port: None,
                led_count: 0,
                color_format: None,
                audio_device: None,
                midi_port: None,
                log_level: None,
            }
        }
    };
    
    // Vytvoření instance naší služby.
    let service = MidiWledService::new(config);
    let binder: Strong<dyn IMidiWledService> = BnMidiWledService::new_binder(service, BinderFeatures::default());

    // Registrace služby u Android Service Manageru.
    match rsbinder::add_service(SERVICE_NAME, binder.as_binder()) {
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
