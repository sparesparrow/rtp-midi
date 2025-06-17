use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use libc::c_char;
use crate::{Config, run_service_loop};
use crate::wled_control;
use tokio::runtime::Runtime;
use log::{error, info};
use once_cell::sync::Lazy;

static TOKIO_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create global Tokio runtime for FFI")
});

/// Opaque struct to hold the state for a running service instance.
/// The pointer to this struct acts as a handle for the C/C++ side.
pub struct ServiceHandle {
    config: Arc<Mutex<Option<Config>>>,
    running: Arc<AtomicBool>,
    worker_thread: Mutex<Option<tokio::task::JoinHandle<()>>>,
    tokio_rt_handle: tokio::runtime::Handle,
}

/// Creates a new service instance but does not start it.
///
/// # Safety
/// The `config_path` pointer must be a valid, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn create_service(config_path: *const c_char) -> *mut ServiceHandle {
    // Initialize logging once
    let _ = env_logger::try_init();

    let path_str = if !config_path.is_null() {
        CStr::from_ptr(config_path).to_str().unwrap_or("")
    } else {
        ""
    };

    let config = match Config::load_from_file(path_str) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            error!("Failed to load config from {}: {}", path_str, e);
            None
        }
    };
    
    let handle = Box::new(ServiceHandle {
        config: Arc::new(Mutex::new(config)),
        running: Arc::new(AtomicBool::new(false)),
        worker_thread: Mutex::new(None),
        tokio_rt_handle: TOKIO_RUNTIME.handle().clone(), // Clone handle from global Runtime
    });

    Box::into_raw(handle)
}

/// Starts the service loop in a background thread.
///
/// # Safety
/// The `handle` must be a valid pointer returned by `create_service`.
#[no_mangle]
pub unsafe extern "C" fn start_service(handle: *mut ServiceHandle) {
    if handle.is_null() { return; }
    let handle_ref = &*handle;

    let mut worker_guard = handle_ref.worker_thread.lock().unwrap();
    if worker_guard.is_some() || handle_ref.running.load(Ordering::SeqCst) {
        info!("Service is already running.");
        return;
    }

    let config_guard = handle_ref.config.lock().unwrap();
    let config = match config_guard.as_ref() {
        Some(c) => c.clone(),
        None => {
            error!("Cannot start service: config is not loaded.");
            return;
        }
    };
    drop(config_guard);

    handle_ref.running.store(true, Ordering::SeqCst);
    let running_clone = handle_ref.running.clone();
    let rt_handle_clone = handle_ref.tokio_rt_handle.clone(); // Use stored handle

    // Spawn the async run_service_loop directly onto the Tokio runtime
    let thread = rt_handle_clone.spawn(async move {
        run_service_loop(config, running_clone).await;
    });

    *worker_guard = Some(thread);
    info!("Service started via FFI.");
}

/// Stops the running service.
///
/// # Safety
/// The `handle` must be a valid pointer returned by `create_service`.
#[no_mangle]
pub unsafe extern "C" fn stop_service(handle: *mut ServiceHandle) {
    if handle.is_null() { return; }
    let handle_ref = &*handle;

    if !handle_ref.running.load(Ordering::SeqCst) {
        info!("Service is not running.");
        return;
    }

    info!("Stopping service via FFI...");
    handle_ref.running.store(false, Ordering::SeqCst);

    let mut worker_guard = handle_ref.worker_thread.lock().unwrap();
    if let Some(thread_handle) = worker_guard.take() {
        // Abort the spawned Tokio task
        thread_handle.abort();
        info!("Service task signaled to stop and handle cleared.");
    }
}

/// Destroys the service instance and frees its memory.
///
/// # Safety
/// The `handle` must be a valid pointer. After this call, the handle is invalid.
#[no_mangle]
pub unsafe extern "C" fn destroy_service(handle: *mut ServiceHandle) {
    if handle.is_null() { return; }
    // Stop the service first to ensure the thread is cleaned up.
    // This will signal the spawned task to stop.
    stop_service(handle);

    // The ServiceHandle is owned by the Box, which will be dropped when it goes out of scope.
    // This will correctly clean up the Arc and other resources.
    let _ = Box::from_raw(handle);
    info!("Service handle destroyed.");
}


/// Sets a WLED preset.
/// # Safety
/// The `handle` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn set_wled_preset(handle: *mut ServiceHandle, preset_id: i32) {
    if handle.is_null() { return; }
    let handle_ref = &*handle;
    
    let config_guard = handle_ref.config.lock().unwrap();
    if let Some(config) = config_guard.as_ref() {
        let ip = config.wled_ip.clone();
        let rt_handle = handle_ref.tokio_rt_handle.clone(); // Use stored handle
        rt_handle.spawn(async move { // Spawn the async task
            if let Err(e) = wled_control::set_wled_preset(&ip, preset_id).await {
                error!("FFI: Failed to set WLED preset: {}", e);
            }
        });
    }
}

/// Gets the WLED IP address from the config.
/// Returns a C string that must be freed with `free_string`.
/// # Safety
/// The `handle` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn get_wled_ip(handle: *mut ServiceHandle) -> *mut c_char {
    if handle.is_null() { return std::ptr::null_mut(); }
    let handle_ref = &*handle;
    let config_guard = handle_ref.config.lock().unwrap();
    if let Some(config) = config_guard.as_ref() {
        if let Ok(s) = CString::new(config.wled_ip.as_str()) {
            return s.into_raw();
        }
    }
    std::ptr::null_mut()
}

/// Frees a C string that was allocated by Rust.
/// # Safety
/// The `s` pointer must have been allocated by Rust (e.g., via CString::into_raw).
#[no_mangle]
pub unsafe extern "C" fn free_string(s: *mut c_char) {
    if s.is_null() { return; }
    let _ = CString::from_raw(s);
}
