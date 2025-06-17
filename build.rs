// build.rs
use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    // Spouštět tento build skript pouze pro Android.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "android" {
        println!("cargo:warning=Skipping AIDL build script: Not targeting Android.");
        return;
    }

    println!("cargo:rerun-if-changed=aidl/com/example/rtpmidi/IMidiWledService.aidl");

    // Definice cest
    let project_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let aidl_file = project_dir.join("aidl/com/example/rtpmidi/IMidiWledService.aidl");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Construct the expected generated Rust file path based on AIDL package and interface name
    // AIDL package: com.example.rtpmidi
    // Interface: IMidiWledService
    let package_path = PathBuf::from("com/example/rtpmidi");
    let interface_name = "IMidiWledService".to_string(); // Use original CamelCase name
    let generated_rust_file_path = out_dir.join(package_path).join(format!("{}.rs", interface_name));

    // Ensure the output directory exists
    fs::create_dir_all(generated_rust_file_path.parent().unwrap())
        .expect("Failed to create output directory for AIDL.");

    if !aidl_file.exists() {
        panic!("AIDL file not found at: {:?}", aidl_file);
    }
    
    println!("cargo:warning=Compiling AIDL file with rsbinder-aidl: {:?}", aidl_file);

    // Spuštění rsbinder-aidl překladače
    rsbinder_aidl::Builder::new()
        .source(&aidl_file)
        .output(&generated_rust_file_path)
        .generate()
        .expect("Failed to execute rsbinder-aidl compiler.");

    if !generated_rust_file_path.exists() {
        panic!("rsbinder-aidl tool ran but did not produce the expected output file: {:?}", generated_rust_file_path);
    }

    println!("cargo:warning=Successfully generated Rust bindings from AIDL to: {}", generated_rust_file_path.display());
}
