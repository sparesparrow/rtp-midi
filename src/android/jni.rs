// src/android/jni.rs

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jint;
use std::thread;
use log::info;
use tokio::runtime::Runtime;
use std::sync::Arc;

use super::aidl_service;

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

    // Vytvoříme jeden Tokio runtime pro celou aplikaci
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
    let rt_handle_clone = rt.handle().clone();

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

    // Získejte handle na existující Tokio runtime (pokud je již inicializován v JNI_OnLoad)
    // nebo vytvořte nový, pokud je tato funkce volána nezávisle.
    // Pro tento případ předpokládáme, že runtime je již připraven a dostupný globálně.
    // V reálné aplikaci byste chtěli předat handle z JNI_OnLoad nebo mít globální Arc<Runtime>.
    // Pro zjednodušení teď vytvoříme nový runtime, ale v budoucnu by se to mělo předat.
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime in nativeInit"));
    let rt_handle = rt.handle().clone();

    thread::spawn(move || {
        aidl_service::register_service(&path, rt_handle);
    });
}
