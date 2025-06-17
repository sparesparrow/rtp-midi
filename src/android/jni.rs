// src/android/jni.rs

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jint;
use std::thread;
use log::info;
use tokio::runtime::Runtime;
use std::sync::Arc;
use once_cell::sync::Lazy;

use super::aidl_service;

static TOKIO_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create global Tokio runtime")
});

/// Tato funkce je volána Androidem, když je knihovna poprvé načtena.
/// Je to hlavní vstupní bod pro inicializaci naší nativní služby.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn JNI_OnLoad(vm: jni::JavaVM, _: std::ffi::c_void) -> jint {
    // Inicializace loggeru pro logování do logcat
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Info)
            .with_tag("RtpMidiRustService"),
    );

    info!("JNI_OnLoad called: Initializing Tokio runtime and spawning service registration thread.");

    let rt_handle_clone = TOKIO_RUNTIME.handle().clone();

    // Registrace služby musí proběhnout v samostatném vlákně,
    // abychom neblokovali `JNI_OnLoad`.
    thread::spawn(move || {
        // Tato funkce se zablokuje a udrží službu naživu.
        // Cesta ke konfiguraci bude předána z Java/Kotlin.
        // Prozatím zde ponecháme placeholder nebo získáme z JNI funkce.
        // aidl_service::register_service("/data/data/com.example.rtpmidi/files/config.toml", rt_handle_clone);
        info!("Service registration will be called from nativeInit.");
    });

    info!("JNI_OnLoad finished, service thread spawned (waiting for nativeInit).");

    // Vracíme verzi JNI, se kterou jsme kompatibilní.
    jni::sys::JNI_VERSION_1_6
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn Java_com_example_rtpmidi_RustServiceWrapper_nativeInit(
    mut env: JNIEnv,
    _: JClass,
    config_path: JString,
) {
    let path: String = env.get_string(&config_path).unwrap().into();
    info!("nativeInit called with config path: {}", path);

    let rt_handle = TOKIO_RUNTIME.handle().clone();

    thread::spawn(move || {
        aidl_service::register_service(&path, rt_handle);
    });
}
