// build.rs
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

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
    
    // Construct the expected generated Rust file path based on AIDL package and interface name
    // AIDL package: com.example.rtpmidi
    // Interface: IMidiWledService
    let package_path = PathBuf::from("com/example/rtpmidi");
    let interface_name = "IMidiWledService".to_string(); // Use original CamelCase name
    let generated_rust_file = out_dir.join(package_path).join(format!("{}.rs", interface_name));

    if !aidl_file.exists() {
        panic!("AIDL file not found at: {:?}", aidl_file);
    }
    
    println!("cargo:warning=Found AIDL tool at: {:?}", aidl_tool_path);
    println!("cargo:warning=Compiling AIDL file: {:?}", aidl_file);

    // 4. Spuštění aidl překladače
    let output = Command::new(&aidl_tool_path)
        .arg(format!("--lang=rust"))
        .arg(format!("-o{}", out_dir.to_str().unwrap())) // Spojeno s -o
        .arg(format!("-I{}", project_dir.join("aidl").to_str().unwrap())) // Add import path
        .arg(aidl_file.to_str().unwrap())
        .output()
        .expect("Failed to execute AIDL compiler.");

    if !output.status.success() {
        eprintln!("AIDL stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("AIDL stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("AIDL compiler failed with status: {}", output.status);
    }

    if !generated_rust_file.exists() {
        panic!("AIDL tool ran but did not produce the expected output file: {:?}", generated_rust_file);
    }

    println!("cargo:warning=Successfully generated Rust bindings from AIDL to: {}", out_dir.display());
}

/// Pomocná funkce pro nalezení 'aidl' nástroje.
fn find_aidl_tool(ndk_path: &PathBuf) -> Option<PathBuf> {
    // Try ANDROID_HOME/build-tools/version/aidl
    if let Ok(android_home) = env::var("ANDROID_HOME") {
        let android_home_path = PathBuf::from(android_home);
        // We'll iterate through common build-tools versions or assume a recent one
        // For simplicity, let's hardcode a recent version for now, or find it dynamically.
        // For dynamic finding, we'd list directories in build-tools.
        let build_tools_path = android_home_path.join("build-tools");
        if let Ok(entries) = fs::read_dir(&build_tools_path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let aidl_path = path.join("aidl");
                        if aidl_path.exists() {
                            return Some(aidl_path);
                        }
                    }
                }
            }
        }
    }

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
