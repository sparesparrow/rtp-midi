use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host};

/// Lists all available input audio devices.
pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.input_devices()?;
    let mut device_names = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            device_names.push(name);
        }
    }
    Ok(device_names)
}

/// Lists all available output audio devices.
pub fn list_output_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.output_devices()?;
    let mut device_names = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            device_names.push(name);
        }
    }
    Ok(device_names)
}

/// Gets the default input device.
pub fn default_input_device() -> Result<Device> {
    let host = cpal::default_host();
    host.default_input_device()
        .ok_or_else(|| anyhow!("No default input device found"))
}

/// Gets the default output device.
pub fn default_output_device() -> Result<Device> {
    let host = cpal::default_host();
    host.default_output_device()
        .ok_or_else(|| anyhow!("No default output device found"))
}

/// Gets an input device by name.
pub fn input_device_by_name(name: &str) -> Result<Device> {
    let host = cpal::default_host();
    host.input_devices()? 
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| anyhow!("Input device not found: {}", name))
}

/// Gets an output device by name.
pub fn output_device_by_name(name: &str) -> Result<Device> {
    let host = cpal::default_host();
    host.output_devices()? 
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| anyhow!("Output device not found: {}", name))
} 