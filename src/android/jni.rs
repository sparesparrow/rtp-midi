// src/android/jni.rs

use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::jint;
use std::thread;
use log::info;

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

    info!("JNI_OnLoad called: Starting service registration thread.");

    // Registrace služby musí proběhnout v samostatném vlákně,
    // abychom neblokovali `JNI_OnLoad`.
    thread::spawn(|| {
        // Tato funkce se zablokuje a udrží službu naživu.
        aidl_service::register_service();
    });

    info!("JNI_OnLoad finished, service thread spawned.");

    // Vracíme verzi JNI, se kterou jsme kompatibilní.
    jni::sys::JNI_VERSION_1_6
}
