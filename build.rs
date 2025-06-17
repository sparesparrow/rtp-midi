// build.rs
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Spouštět tento build skript pouze pro Android.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "android" {
        println!("cargo:warning=Skipping AIDL build script: Not targeting Android.");
        return;
    }

    println!("cargo:rerun-if-changed=aidl/com/example/rtpmidi/IMidiWledService.aidl");

    // 1. Získání cesty k Android NDK
    let android_ndk_home = match env::var("ANDROID_NDK_HOME") {
        Ok(val) => PathBuf::from(val),
        Err(_) => {
            println!("cargo:warning=ANDROID_NDK_HOME is not set. Please set this environment variable to your NDK path. Skipping AIDL compilation.");
            return;
        }
    };

    // 2. Nalezení aidl překladače v NDK
    let aidl_tool_path = find_aidl_tool(&android_ndk_home)
        .expect("Failed to find 'aidl' executable in NDK. Please check your NDK installation.");

    // 3. Definice cest
    let project_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let aidl_file = project_dir.join("aidl/com/example/rtpmidi/IMidiWledService.aidl");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Cílový soubor, který bude vygenerován
    let generated_rust_file = out_dir.join("com_example_rtpmidi_IMidiWledService.rs");

    if !aidl_file.exists() {
        panic!("AIDL file not found at: {:?}", aidl_file);
    }
    
    println!("cargo:warning=Found AIDL tool at: {:?}", aidl_tool_path);
    println!("cargo:warning=Compiling AIDL file: {:?}", aidl_file);

    // 4. Spuštění aidl překladače
    let status = Command::new(&aidl_tool_path)
        .arg(format!("--lang=rust"))
        .arg(format!("-o{}", out_dir.to_str().unwrap())) // Spojeno s -o
        .arg(aidl_file.to_str().unwrap())
        .status()
        .expect("Failed to execute AIDL compiler.");

    if !status.success() {
        panic!("AIDL compiler failed with status: {}", status);
    }

    if !generated_rust_file.exists() {
        panic!("AIDL tool ran but did not produce the expected output file: {:?}", generated_rust_file);
    }

    println!("cargo:warning=Successfully generated Rust bindings from AIDL to: {}", out_dir.display());
}

/// Pomocná funkce pro nalezení 'aidl' nástroje v NDK.
fn find_aidl_tool(ndk_path: &PathBuf) -> Option<PathBuf> {
    let host_os = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };

    // Běžná cesta pro moderní NDK verze
    let tool_path = ndk_path.join("build/tools/aidl_tool");
    if tool_path.exists() {
        return Some(tool_path.join(host_os).join("bin/aidl"));
    }
    
    // Starší cesta
    let tool_path = ndk_path.join("prebuilt").join(host_os).join("bin/aidl");
    if tool_path.exists() {
        return Some(tool_path);
    }

    None
}
