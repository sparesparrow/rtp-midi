use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

use log::{error, info};

// --- Modular Crate Imports ---
use audio::audio_analysis::compute_fft_magnitudes;
use audio::audio_input;
use rtp_midi_core::{event_bus, DataStreamNetSender};
use utils::{MidiCommand, parse_midi_message, InputEvent, Mapping};
use network::midi::rtp::message::MidiMessage;
use network::midi::rtp::session::RtpMidiSession;
use output::wled_control::WledSender;
use tokio::sync::Mutex;

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
/// Hlavní service loop pro orchestraci audio/MIDI vstupů a výstupů.
/// 
/// Výstupy (WLED, DDP, ...) používejte přes sjednocené API (DataStreamNetSender).
/// Pro rozšíření mappingů o další typy akcí/výstupů:
///   - Přidejte nový enum (např. DdpOutputAction) do utils.
///   - Přidejte nový sender (např. DdpSender) a implementujte DataStreamNetSender.
///   - V service loop směrujte akce na správný výstup podle typu.
///
/// Příklad rozšíření:
/// if let MappingOutput::Wled(action) = ... { wled_sender.send(...); }
/// if let MappingOutput::Ddp(action) = ... { ddp_sender.send(...); }
pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let midi_port = config.midi_port.unwrap_or(5004);
    let mappings = config.mappings.clone();
    let ddp_port = config.ddp_port.unwrap_or(4048);

    // --- Výstupní zařízení ---
    let mut wled_sender = WledSender::new(wled_ip.clone());
    // DDP sender lze přidat obdobně, až budou akce
    // let mut ddp_sender = ...

    // --- Event Bus Setup ---
    let event_bus = event_bus::EventBus::new(32);
    let event_tx = event_bus.sender.clone();
    let mut audio_event_rx = event_tx.subscribe();
    let mut midi_event_rx = event_tx.subscribe();
    let network_send_rx = event_tx.subscribe();
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
        let session = Arc::new(Mutex::new(RtpMidiSession::new("Rust WLED Hub".to_string(), midi_port)
            .await
            .expect("Failed to create RTP-MIDI session")));

        let mut raw_packet_rx = event_tx_clone_midi.subscribe();
        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi1 = event_tx_clone_midi.clone();
        tokio::spawn(async move {
            while let Ok(event) = raw_packet_rx.recv().await {
                if let event_bus::Event::RawPacketReceived { payload, source_addr } = event {
                    info!("RTP-MIDI Session received raw packet from {}: {:?}", source_addr, payload);
                    session_clone.lock().await.handle_incoming_packet(payload).await;
                }
            }
        });

        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi2 = event_tx_clone_midi.clone();
        session.lock().await.add_outgoing_packet_handler(move |destination, port, data| {
            let ip: IpAddr = destination.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
            let dest_addr = SocketAddr::new(ip, port);
            if let Err(e) = event_tx_clone_midi2.send(event_bus::Event::SendPacket {
                payload: data.to_vec(),
                dest_addr,
            }) {
                error!("Failed to send outgoing RTP-MIDI packet to event bus: {}", e);
            }
        }).await;

        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi3 = event_tx_clone_midi.clone();
        session.lock().await.add_midi_command_handler(move |commands: Vec<MidiMessage>| {
            for command in commands {
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
                if let Err(e) = event_tx_clone_midi3.send(event_bus::Event::MidiCommandsReceived {
                    commands: command.command.clone(),
                    timestamp,
                    peer,
                }) {
                    error!("Failed to send MIDI command to event bus: {}", e);
                }
            }
        }).await;

        info!(
            "RTP-MIDI Server started on port {}. Waiting for connections...",
            midi_port
        );
        // let _ = session.start().await; // No start() method; session is ready after construction
    });

    // --- Main Processing Loop ---
    let mut prev_mags = Vec::new();
    let mut bass_preset_triggered = false;

    while running.load(Ordering::SeqCst) {
        // --- Audio Processing ---
        if let Ok(event) = audio_event_rx.try_recv() {
            if let event_bus::Event::AudioDataReady(audio_buffer) = event {
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
                                        match action {
                                            utils::MappingOutput::Wled(wled_action) => {
                                                if let Ok(payload) = serde_json::to_vec(&wled_action) {
                                                    let _ = wled_sender.send(0, &payload);
                                                }
                                            }
                                            // utils::MappingOutput::Ddp(ddp_action) => {
                                            //     // Přidejte logiku pro DDP výstup
                                            // }
                                        }
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
        if let Ok(event) = midi_event_rx.try_recv() {
            if let event_bus::Event::MidiCommandsReceived { commands, timestamp, peer } = event {
                if let Ok((parsed_command, _)) = parse_midi_message(&commands) {
                    if let Some(mappings) = &mappings {
                        for mapping in mappings {
                            if mapping.matches_midi_command(&parsed_command) {
                                match parsed_command {
                                    MidiCommand::NoteOn { key, .. } => info!("MIDI NoteOn {} matched a mapping.", key),
                                    MidiCommand::ControlChange { control, value, ..} => info!("MIDI CC {} ({}) matched a mapping.", control, value),
                                    _ => ()
                                }
                                for action in &mapping.output {
                                    match action {
                                        utils::MappingOutput::Wled(wled_action) => {
                                            if let Ok(payload) = serde_json::to_vec(&wled_action) {
                                                let _ = wled_sender.send(0, &payload);
                                            }
                                        }
                                        // utils::MappingOutput::Ddp(ddp_action) => {
                                        //     // Přidejte logiku pro DDP výstup
                                        // }
                                    }
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

