use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

// --- Core Modules & Imports ---
use crate::audio_analysis::compute_fft_magnitudes;
use crate::mapping::{InputEvent, Mapping, Config};
use crate::midi::parser::{self, MidiCommand};
use crate::midi::rtp::message::MidiMessage;
use crate::midi::rtp::session::RtpMidiSession;
use crate::wled_control as wled;


// --- Module Declarations ---
pub mod android;
pub mod audio_analysis;
pub mod audio_input;
pub mod ddp_output;
pub mod ffi;
pub mod light_mapper;
pub mod mapping;
pub mod midi;
pub mod wled_control;


// --- Main Service Loop ---
pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let midi_port = config.midi_port.unwrap_or(5004);
    let mappings = config.mappings.clone();

    // --- Channel Setup ---
    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();
    let (midi_tx, midi_rx) = crossbeam_channel::unbounded();

    // --- Start Audio Input Thread ---
    let audio_input_config = config.clone();
    let _audio_input_handle = std::thread::spawn(move || {
        audio_input::start_audio_input(audio_input_config.audio_device.as_deref(), audio_tx)
            .expect("Failed to start audio input stream");
    });

    // --- Start RTP-MIDI Session Task ---
    let midi_tx_clone = midi_tx.clone();
    tokio::spawn(async move {
        let session = RtpMidiSession::new("Rust WLED Hub".to_string(), midi_port)
            .await
            .expect("Failed to create RTP-MIDI session");

        // The listener now correctly accepts a `Vec<MidiMessage>`.
        session.add_listener(move |commands: Vec<MidiMessage>| {
            for command in commands {
                if let Err(e) = midi_tx_clone.send(command) {
                    error!("Failed to send MIDI command to channel: {}", e);
                }
            }
        });

        info!(
            "RTP-MIDI Server started on port {}. Waiting for connections...",
            midi_port
        );
        let _ = session.start().await;
    });


    // --- Main Processing Loop ---
    let mut prev_mags = Vec::new();
    let mut bass_preset_triggered = false;

    while running.load(Ordering::SeqCst) {
        // --- Audio Processing ---
        if let Ok(audio_buffer) = audio_rx.try_recv() {
            let magnitudes = compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);
            let band_size = magnitudes.len() / 3;
            let bass_level = magnitudes.iter().take(band_size).cloned().fold(0.0, f32::max);

            if let Some(mappings) = &mappings {
                for mapping in mappings {
                    if let InputEvent::AudioBand { band, threshold } = &mapping.input {
                        if band == "bass" {
                            let trigger_threshold = threshold.unwrap_or(0.8);

                            if bass_level >= trigger_threshold && !bass_preset_triggered {
                                info!("Bass peak detected! Level: {:.2}. Triggering actions.", bass_level);
                                for action in &mapping.output {
                                    wled::execute_wled_action(action, &wled_ip).await;
                                }
                                bass_preset_triggered = true;
                            } else if bass_level < trigger_threshold && bass_preset_triggered {
                                bass_preset_triggered = false;
                            }
                        }
                    }
                }
            }
        }

        // --- MIDI Processing ---
        if let Ok(event) = midi_rx.try_recv() { // event is MidiMessage
            // FIX: Parse the raw command from the message into a MidiCommand enum
            if let Ok((parsed_command, _)) = parser::parse_midi_message(&event.command) {
                if let Some(mappings) = &mappings {
                    for mapping in mappings {
                        // Use the parsed command for matching
                        if mapping.matches_midi_command(&parsed_command) {
                            // Use the parsed command for the match statement
                            match parsed_command {
                                MidiCommand::NoteOn { key, .. } => info!("MIDI NoteOn {} matched a mapping.", key),
                                MidiCommand::ControlChange { control, value, ..} => info!("MIDI CC {} ({}) matched a mapping.", control, value),
                                _ => ()
                            }
                            for action in &mapping.output {
                                wled::execute_wled_action(action, &wled_ip).await;
                            }
                        }
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    info!("Service has shut down gracefully.");
}
