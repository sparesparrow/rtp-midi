// tests/cli_tests.rs

use std::process::Command;

#[test]
fn test_help_message() {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--") // Pass arguments to the binary
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    // Check that the command ran successfully
    assert!(output.status.success());

    // Check that the help message contains expected text
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rtp-midi")); // Replace "rtp-midi" with your binary name
}

#[test]
fn test_invalid_argument() {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--role")
        .arg("invalid")
        .output()
        .expect("Failed to execute command");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Použití") || stderr.contains("Usage"));
}

#[test]
fn test_missing_config_for_server() {
    // Temporarily rename config.toml if it exists
    let config_path = "config.toml";
    let backup_path = "config.toml.bak";
    let config_exists = std::path::Path::new(config_path).exists();
    if config_exists {
        std::fs::rename(config_path, backup_path).unwrap();
    }
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--role")
        .arg("server")
        .output()
        .expect("Failed to execute command");
    if config_exists {
        std::fs::rename(backup_path, config_path).unwrap();
    }
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config.toml") || stderr.contains("načtení selhalo"));
}

#[test]
fn test_ui_host_role() {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--role")
        .arg("ui-host")
        .output()
        .expect("Failed to execute command");
    // Should print UI server info or error if port in use
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("UI dostupné") || stdout.contains("Spouštím v režimu UI-HOST"));
}

#[test]
fn test_client_role() {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--role")
        .arg("client")
        .output()
        .expect("Failed to execute command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Spouštte klientskou aplikaci") || stdout.contains("Spouštím v režimu CLIENT"));
}

#[test]
fn test_server_role_with_config() {
    // Only run if config.toml exists
    if !std::path::Path::new("config.toml").exists() {
        eprintln!("Skipping test_server_role_with_config: config.toml not found");
        return;
    }
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--role")
        .arg("server")
        .output()
        .expect("Failed to execute command");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Spouštím v režimu SERVER"));
}

// Add more tests for different commands, arguments, and scenarios
