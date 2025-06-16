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

// Add more tests for different commands, arguments, and scenarios
