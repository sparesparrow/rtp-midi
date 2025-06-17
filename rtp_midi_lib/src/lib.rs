use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

// --- Modular Crate Imports ---
use audio::audio_analysis::compute_fft_magnitudes;
use audio::audio_input;
use core::{event_bus, mapping::{InputEvent, Mapping}};
use network::midi::parser::{self, MidiCommand};
use network::midi::rtp::message::MidiMessage;
use network::midi::rtp::session::RtpMidiSession;
use output as wled;

// --- Structs defined at the library root ---
#[derive(Debug, serde::Deserialize, Clone, PartialEq)]
pub struct Config {
    pub wled_ip: String,
    pub ddp_port: Option<u16>,
    pub led_count: usize,
    pub color_format: Option<String>,
    pub audio_device: Option<String>,
    pub midi_port: Option<u16>,
    pub log_level: Option<String>,
    pub mappings: Option<Vec<Mapping>>,
    pub signaling_server_address: String,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub audio_buffer_size: usize,
    pub audio_smoothing_factor: f32,
    pub webrtc_ice_servers: Option<Vec<String>>,
}

// --- Implementation for Config ---
impl Config {
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

// --- Main Service Loop ---
pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let midi_port = config.midi_port.unwrap_or(5004);
    let mappings = config.mappings.clone();
    let ddp_port = config.ddp_port.unwrap_or(4048);

    // --- Event Bus Setup ---
    let (event_tx, mut audio_event_rx) = event_bus::create_event_bus();
    let mut midi_event_rx = event_tx.subscribe();
    let mut network_send_rx = event_tx.subscribe();
    let network_recv_tx = event_tx.clone();

    // --- Start Network Interface Task ---
    tokio::spawn(async move {
        network::network_interface::start_network_interface(network_send_rx, network_recv_tx, midi_port)
            .await
            .expect("Failed to start network interface");
    });

    // --- Start Audio Input Thread ---
    let audio_input_config = config.clone();
    let event_tx_clone = event_tx.clone();
    let _audio_input_handle = std::thread::spawn(move || {
        audio_input::start_audio_input(audio_input_config.audio_device.as_deref(), event_tx_clone)
            .expect("Failed to start audio input stream");
    });

    // --- Start RTP-MIDI Session Task ---
    let event_tx_clone_midi = event_tx.clone();
    tokio::spawn(async move {
        let mut session = RtpMidiSession::new("Rust WLED Hub".to_string(), midi_port)
            .await
            .expect("Failed to create RTP-MIDI session");

        let mut raw_packet_rx = event_tx_clone_midi.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = raw_packet_rx.recv().await {
                if let core::event_bus::Event::RawPacketReceived { source, data } = event {
                    info!("RTP-MIDI Session received raw packet from {}: {:?}", source, data);
                    session.handle_incoming_packet(data).await;
                }
            }
        });

        session.add_outgoing_packet_handler(move |destination, port, data| {
            if let Err(e) = event_tx_clone_midi.send(core::event_bus::Event::SendPacket { 
                destination: destination.to_string(), 
                port, 
                data 
            }) {
                error!("Failed to send outgoing RTP-MIDI packet to event bus: {}", e);
            }
        }).await;

        session.add_midi_command_handler(move |commands: Vec<MidiMessage>| {
            for command in commands {
                if let Err(e) = event_tx_clone_midi.send(core::event_bus::Event::MidiMessageReceived(command.command)) {
                    error!("Failed to send MIDI command to event bus: {}", e);
                }
            }
        }).await;

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
        if let Ok(event) = audio_event_rx.try_recv() {
            if let core::event_bus::Event::AudioDataReady(audio_buffer) = event {
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
                                        wled::wled_control::execute_wled_action(action, &wled_ip).await;
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
        }

        // --- MIDI Processing ---
        if let Ok(event) = midi_event_rx.try_recv() { // event is MidiMessage
            if let core::event_bus::Event::MidiMessageReceived(midi_command_bytes) = event {
                if let Ok((parsed_command, _)) = parser::parse_midi_message(&midi_command_bytes) {
                    if let Some(mappings) = &mappings {
                        for mapping in mappings {
                            if mapping.matches_midi_command(&parsed_command) {
                                match parsed_command {
                                    MidiCommand::NoteOn { key, .. } => info!("MIDI NoteOn {} matched a mapping.", key),
                                    MidiCommand::ControlChange { control, value, ..} => info!("MIDI CC {} ({}) matched a mapping.", control, value),
                                    _ => ()
                                }
                                for action in &mapping.output {
                                    wled::wled_control::execute_wled_action(action, &wled_ip).await;
                                }
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

