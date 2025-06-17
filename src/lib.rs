use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

use crate::audio_analysis::compute_fft_magnitudes;
use crate::mapping::{Config, InputEvent};
use crate::midi::parser::MidiCommand;
use crate::midi::rtp::message::RtpMidiPacket as RtpMidiSession;
use crate::wled_control as wled;

pub mod android;
pub mod audio_analysis;
pub mod audio_input;
pub mod ddp_output;
pub mod ffi;
pub mod light_mapper;
pub mod mapping;
pub mod midi;
pub mod wled_control;

pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let midi_port = config.midi_port.unwrap_or(5004);
    let mappings = config.mappings.clone();

    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();
    let (_midi_tx, midi_rx) = crossbeam_channel::unbounded();

    let audio_input_config = config.clone();
    let _audio_input_handle = std::thread::spawn(move || {
        audio_input::start_audio_input(
            audio_input_config.audio_device.as_deref(),
            audio_tx
        )
        .expect("Failed to start audio input stream");
    });

    let midi_tx_clone = _midi_tx.clone();
    tokio::spawn(async move {
        let session = RtpMidiSession::new("Rust WLED Hub".to_string(), midi_port)
            .await
            .expect("Failed to create RTP-MIDI session");

        session.add_listener(move |packet| {
            for command in packet.midi_commands() {
                if let Err(e) = midi_tx_clone.send(command.clone()) {
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

    let mut prev_mags = Vec::new();
    let mut bass_preset_triggered = false;

    while running.load(Ordering::SeqCst) {
        if let Ok(audio_buffer) = audio_rx.try_recv() {
            let magnitudes = compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);
            let band_size = magnitudes.len() / 3;
            let bass_level = magnitudes
                .iter()
                .take(band_size)
                .cloned()
                .fold(0.0, f32::max);

            if let Some(mappings) = &mappings {
                for mapping in mappings {
                    if let InputEvent::AudioBand { band, threshold } = &mapping.input {
                        if band == "bass" {
                            let trigger_threshold = threshold.unwrap_or(0.8);

                            if bass_level >= trigger_threshold && !bass_preset_triggered {
                                info!(
                                    "Bass peak detected! Level: {:.2}. Triggering actions.",
                                    bass_level
                                );
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

        if let Ok(event) = midi_rx.try_recv() {
            if let Some(mappings) = &mappings {
                for mapping in mappings {
                    if mapping.matches_midi_command(&event) {
                        if let MidiCommand::NoteOn { key, .. } = event {
                            info!("MIDI NoteOn {} matched a mapping.", key);
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
