use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use crossbeam_channel::Sender;
use log::{error, info, warn};
use num_traits;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;

pub mod android;
pub mod ffi;
pub mod audio_input;
pub mod light_mapper;
pub mod ddp_output;
pub mod midi;
pub mod mapping;

pub use midi::rtp::message::{MidiMessage, RtpMidiPacket, MidiCommand};
pub use mapping::InputEvent;

/// Application configuration loaded from config.toml
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Config {
    /// IP address of the WLED controller (for both DDP and JSON API)
    pub wled_ip: String,
    /// UDP port for DDP (default: 4048)
    pub ddp_port: Option<u16>,
    /// Number of LEDs (must match WLED config)
    pub led_count: usize,
    /// RGB or RGBW (3 or 4 channels per pixel)
    pub color_format: Option<String>,
    /// Audio input device name (optional, default: system default)
    pub audio_device: Option<String>,
    /// MIDI RTP port (default: 5004)
    pub midi_port: Option<u16>,
    /// Log level (info, debug, etc.)
    pub log_level: Option<String>,
    /// Advanced mappings from input events to WLED actions
    pub mappings: Option<Vec<mapping::Mapping>>,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "wled_ip = \"127.0.0.1\"\nddp_port = 4048\nled_count = 10\ncolor_format = \"RGB\"\naudio_device = \"default\"\nmidi_port = 5004\nlog_level = \"info\"").unwrap();
        let config = Config::load_from_file(file.path()).unwrap();
        assert_eq!(config.wled_ip, "127.0.0.1");
        assert_eq!(config.ddp_port, Some(4048));
        assert_eq!(config.led_count, 10);
        assert_eq!(config.color_format.as_deref(), Some("RGB"));
        assert_eq!(config.audio_device.as_deref(), Some("default"));
        assert_eq!(config.midi_port, Some(5004));
        assert_eq!(config.log_level.as_deref(), Some("info"));
    }

    #[test]
    fn test_load_invalid_file() {
        let res = Config::load_from_file("/nonexistent/path/to/config.toml");
        assert!(res.is_err());
    }
}

pub mod wled_control;

pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let audio_device_name = config.audio_device.clone();
    let mappings = config.mappings.clone();

    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();
    let (midi_tx, midi_rx) = crossbeam_channel::unbounded();
    let (input_event_tx, input_event_rx) = crossbeam_channel::unbounded();

    // Audio Input Thread
    let audio_input_config = config.clone();
    let audio_input_handle = std::thread::spawn(move || {
        audio_input::start_audio_input(audio_input_config.audio_device.as_deref(), audio_tx)
            .expect("Failed to start audio input stream");
    });

    // Main service loop for audio processing and DDP output
    let mut prev_mags = Vec::new();
    let mut ddp_sender = ddp_output::create_ddp_sender(
        &wled_ip,
        config.ddp_port.unwrap_or(4048),
        config.led_count,
        false // is_rgbw
    ).expect("Failed to create DDP sender");

    let mut last_peak_time = std::time::Instant::now();
    let peak_debounce_duration = Duration::from_millis(100);

    while running.load(Ordering::SeqCst) {
        // Process audio data
        if let Ok(audio_buffer) = audio_rx.try_recv() {
            let magnitudes = light_mapper::compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);

            // AudioPeak detection
            let current_time = std::time::Instant::now();
            let overall_magnitude: f32 = magnitudes.iter().sum::<f32>() / magnitudes.len() as f32;
            if overall_magnitude > 0.6 && current_time.duration_since(last_peak_time) > peak_debounce_duration {
                if let Err(e) = input_event_tx.send(InputEvent::AudioPeak) {
                    error!("Failed to send AudioPeak event: {}", e);
                }
                last_peak_time = current_time;
            }

            // AudioBand events
            let band_size = magnitudes.len() / 3;
            let bass = magnitudes.iter().take(band_size).cloned().fold(0.0, f32::max);
            let mid = magnitudes.iter().skip(band_size).take(band_size).cloned().fold(0.0, f32::max);
            let treble = magnitudes.iter().skip(2 * band_size).cloned().fold(0.0, f32::max);

            if bass > 0.7 {
                if let Err(e) = input_event_tx.send(InputEvent::AudioBand { band: "bass".to_string(), threshold: Some(bass) }) {
                    error!("Failed to send AudioBand (bass) event: {}", e);
                }
            }
            if mid > 0.7 {
                if let Err(e) = input_event_tx.send(InputEvent::AudioBand { band: "mid".to_string(), threshold: Some(mid) }) {
                    error!("Failed to send AudioBand (mid) event: {}", e);
                }
            }
            if treble > 0.7 {
                if let Err(e) = input_event_tx.send(InputEvent::AudioBand { band: "treble".to_string(), threshold: Some(treble) }) {
                    error!("Failed to send AudioBand (treble) event: {}", e);
                }
            }

            let leds = light_mapper::map_audio_to_leds(&magnitudes, config.led_count);
            if let Err(e) = ddp_output::send_ddp_frame(&mut ddp_sender, &leds) {
                error!("Failed to send DDP frame: {}", e);
            }
        }

        // Process MIDI and InputEvent data
        while let Ok(event) = midi_rx.try_recv() {
            info!("Processing MIDI command: {:?}", event);
            // This block now expects MidiCommand, but we need to unify with InputEvent.
            // For now, I'll keep it as MidiCommand, and add a separate loop for InputEvent.
            // In a later step, we can create a unified Event enum.
            if let MidiCommand::NoteOn { key, .. } = event {
                let preset_id = (key as i32 - 47).max(1); 
                info!("Attempting to set WLED preset {} from MIDI note {}", preset_id, key);
                if let Err(e) = wled_control::set_wled_preset(&wled_ip, preset_id).await {
                    error!("Failed to set WLED preset: {}", e);
                }
            } else {
                if let Some(mappings) = &mappings {
                    for mapping in mappings {
                        if mapping.matches_midi_command(&event) {
                            info!("MIDI command matched a mapping, executing WLED actions.");
                            for action in &mapping.output {
                                match action {
                                    mapping::WledOutputAction::SetPreset { id } => {
                                        if let Err(e) = wled_control::set_wled_preset(&wled_ip, *id).await {
                                            error!("Failed to set WLED preset {}: {}", id, e);
                                        }
                                    },
                                    mapping::WledOutputAction::SetBrightness { value } => {
                                        if let Err(e) = wled_control::set_wled_brightness(&wled_ip, *value).await {
                                            error!("Failed to set WLED brightness {}: {}", value, e);
                                        }
                                    },
                                    mapping::WledOutputAction::SetColor { r, g, b } => {
                                        if let Err(e) = wled_control::set_wled_color(&wled_ip, *r, *g, *b).await {
                                            error!("Failed to set WLED color: R={}, G={}, B={}: {}", r, g, b, e);
                                        }
                                    },
                                    mapping::WledOutputAction::SetEffect { id, speed, intensity } => {
                                        if let Err(e) = wled_control::set_wled_effect(&wled_ip, *id, *speed, *intensity).await {
                                            error!("Failed to set WLED effect {}: {}", id, e);
                                        }
                                    },
                                    mapping::WledOutputAction::SetPalette { id } => {
                                        if let Err(e) = wled_control::set_wled_palette(&wled_ip, *id).await {
                                            error!("Failed to set WLED palette {}: {}", id, e);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }

        // Process InputEvent data
        while let Ok(input_event) = input_event_rx.try_recv() {
            info!("Processing InputEvent: {:?}", input_event);
            if let Some(mappings) = &mappings {
                for mapping in mappings {
                    if mapping.input == input_event {
                        info!("InputEvent matched a mapping, executing WLED actions.");
                        for action in &mapping.output {
                            match action {
                                mapping::WledOutputAction::SetPreset { id } => {
                                    if let Err(e) = wled_control::set_wled_preset(&wled_ip, *id).await {
                                        error!("Failed to set WLED preset {}: {}", id, e);
                                    }
                                },
                                mapping::WledOutputAction::SetBrightness { value } => {
                                    if let Err(e) = wled_control::set_wled_brightness(&wled_ip, *value).await {
                                        error!("Failed to set WLED brightness {}: {}", value, e);
                                    }
                                },
                                mapping::WledOutputAction::SetColor { r, g, b } => {
                                    if let Err(e) = wled_control::set_wled_color(&wled_ip, *r, *g, *b).await {
                                        error!("Failed to set WLED color: R={}, G={}, B={}: {}", r, g, b, e);
                                    }
                                },
                                mapping::WledOutputAction::SetEffect { id, speed, intensity } => {
                                    if let Err(e) = wled_control::set_wled_effect(&wled_ip, *id, *speed, *intensity).await {
                                        error!("Failed to set WLED effect {}: {}", id, e);
                                    }
                                },
                                mapping::WledOutputAction::SetPalette { id } => {
                                    if let Err(e) = wled_control::set_wled_palette(&wled_ip, *id).await {
                                        error!("Failed to set WLED palette {}: {}", id, e);
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }

        // Yield control to the Tokio scheduler briefly
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    info!("Service loop stopped.");
    audio_input_handle.join().expect("Could not join audio input thread");
}

// Temporary placeholder for color conversion, ideally in a separate util module
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0) as u32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        5 => (v, p, q),
        _ => (v, p, q), // Should not happen
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
