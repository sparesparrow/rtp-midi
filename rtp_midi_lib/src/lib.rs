use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{error, info};

// --- Modular Crate Imports ---
use audio::audio_analysis::compute_fft_magnitudes;
use audio::audio_input;
use network::midi::rtp::message::MidiMessage;
use network::midi::rtp::session::RtpMidiSession;
use output::ddp_output::{create_ddp_sender, DdpReceiver, DdpSender};
use output::light_mapper::{map_leds_with_preset, MappingPreset};
use output::wled_control::WledSender;
use rtp_midi_core::{event_bus, DataStreamNetReceiver, DataStreamNetSender};
use rtp_midi_core::{parse_midi_message, InputEvent, Mapping, MappingOutput, MidiCommand};
use tokio::sync::watch;
use tokio::sync::Mutex;

// --- Structs defined at the library root ---

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
pub async fn run_service_loop(config: Config, mut shutdown_rx: watch::Receiver<bool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let midi_port = config.midi_port.unwrap_or(5004);
    let mappings = config.mappings.clone();
    let ddp_port = config.ddp_port.unwrap_or(4048);

    // --- Výstupní zařízení ---
    let mut wled_sender = WledSender::new(wled_ip.clone());
    let ddp_ip = wled_ip.clone();
    let mut ddp_sender = match create_ddp_sender(&ddp_ip, ddp_port, config.led_count, false) {
        Ok(sender) => DdpSender::new(sender),
        Err(e) => {
            error!("Failed to create DDP sender: {}", e);
            panic!("Failed to create DDP sender: {}", e);
        }
    };

    // --- DDP Receiver Thread ---
    let ddp_shutdown_rx = shutdown_rx.clone();
    let ddp_task = tokio::spawn(async move {
        let mut receiver = DdpReceiver::new();
        if let Err(e) = receiver.init() {
            error!("Failed to initialize DDP receiver: {}", e);
            return;
        }
        let mut buf = [0u8; 2048];
        let mut shutdown_recv = ddp_shutdown_rx.clone();
        loop {
            tokio::select! {
                _ = shutdown_recv.changed() => {
                    if *shutdown_recv.borrow() {
                        info!("DDP receiver thread shutting down.");
                        break;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {
                    match receiver.poll(&mut buf) {
                        Ok(Some((ts, len))) => {
                            info!("Received DDP frame: timestamp={}ms, len={}", ts, len);
                        }
                        Ok(None) => {
                            // No data available, continue
                        }
                        Err(e) => {
                            error!("DDP receiver error: {}", e);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    });

    // --- Event Bus Setup ---
    let event_bus = event_bus::EventBus::new(32);
    let event_tx = event_bus.sender.clone();
    let mut audio_event_rx = event_tx.subscribe();
    let mut midi_event_rx = event_tx.subscribe();
    let network_send_rx = event_tx.subscribe();
    let network_recv_tx = event_tx.clone();

    // --- Unified Shutdown Channel ---
    // The shutdown channel is now passed in from the entry point (CLI/FFI)

    // --- Start Network Interface Task ---
    let mut network_shutdown_rx = shutdown_rx.clone();
    let network_task = tokio::spawn(async move {
        network::network_interface::start_network_interface(
            network_send_rx,
            network_recv_tx,
            midi_port,
            &mut network_shutdown_rx,
        )
        .await
        .expect("Failed to start network interface");
    });

    // --- Start Audio Input Thread ---
    let audio_input_config = config.clone();
    let event_tx_clone = event_tx.clone();
    let audio_stream = match audio_input::start_audio_input(
        audio_input_config.audio_device.as_deref(),
        event_tx_clone,
    ) {
        Ok(stream) => stream,
        Err(e) => {
            error!("Failed to start audio input stream: {}", e);
            return;
        }
    };
    // Keep the stream alive by storing it in a variable that will be dropped when the function ends
    let _audio_stream_guard = audio_stream;

    // --- Start RTP-MIDI Session Task ---
    let event_tx_clone_midi = event_tx.clone();
    tokio::spawn(async move {
        let session = Arc::new(Mutex::new(
            RtpMidiSession::new("Rust WLED Hub".to_string(), midi_port)
                .await
                .expect("Failed to create RTP-MIDI session"),
        ));

        let mut raw_packet_rx = event_tx_clone_midi.subscribe();
        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi_inner = event_tx_clone_midi.clone();
        tokio::spawn(async move {
            while let Ok(event) = raw_packet_rx.recv().await {
                if let event_bus::Event::RawPacketReceived {
                    payload,
                    source_addr,
                } = event
                {
                    info!(
                        "RTP-MIDI Session received raw packet from {}: {:?}",
                        source_addr, payload
                    );
                    session_clone
                        .lock()
                        .await
                        .handle_incoming_packet(payload, &event_tx_clone_midi_inner, source_addr)
                        .await;
                }
            }
        });

        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi2 = event_tx_clone_midi.clone();
        session
            .lock()
            .await
            .add_outgoing_packet_handler(move |destination, port, data| {
                let ip: IpAddr = destination
                    .parse()
                    .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
                let dest_addr = SocketAddr::new(ip, port);
                if let Err(e) = event_tx_clone_midi2.send(event_bus::Event::SendPacket {
                    payload: data.to_vec(),
                    dest_addr,
                }) {
                    error!(
                        "Failed to send outgoing RTP-MIDI packet to event bus: {}",
                        e
                    );
                }
            })
            .await;

        let session_clone = Arc::clone(&session);
        let event_tx_clone_midi3 = event_tx_clone_midi.clone();
        session
            .lock()
            .await
            .add_midi_command_handler(move |commands: Vec<MidiMessage>| {
                for command in commands {
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
                    if let Err(e) =
                        event_tx_clone_midi3.send(event_bus::Event::MidiCommandsReceived {
                            commands: command.command.clone(),
                            timestamp,
                            peer,
                        })
                    {
                        error!("Failed to send MIDI command to event bus: {}", e);
                    }
                }
            })
            .await;

        info!(
            "RTP-MIDI Server started on port {}. Waiting for connections...",
            midi_port
        );
        // let _ = session.start().await; // No start() method; session is ready after construction
    });

    // --- Main Processing Loop ---
    let mut prev_mags = Vec::new();
    let mut bass_preset_triggered = false;
    let mapping_preset = match config.mapping_preset.as_deref() {
        Some("vumeter") => MappingPreset::VuMeter,
        _ => MappingPreset::Spectrum,
    };

    while !*shutdown_rx.borrow() {
        // --- Audio Processing ---
        if let Ok(event) = audio_event_rx.try_recv() {
            if let event_bus::Event::AudioDataReady(audio_buffer) = event {
                let magnitudes = compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);
                let band_size = magnitudes.len() / 3;
                let bass_level = magnitudes
                    .iter()
                    .take(band_size)
                    .cloned()
                    .fold(0.0, f32::max);
                let led_data = map_leds_with_preset(&magnitudes, config.led_count, mapping_preset);
                // Send led_data to DDP output
                if let Err(e) = ddp_sender.send(0, &led_data) {
                    error!("Failed to send LED data to DDP output: {}", e);
                }

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
                                        match action {
                                            MappingOutput::Wled(wled_action) => {
                                                if let Ok(payload) =
                                                    serde_json::to_vec(&wled_action)
                                                {
                                                    let _ = wled_sender.send(0, &payload);
                                                }
                                            } // utils::MappingOutput::Ddp(ddp_action) => {
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
            if let event_bus::Event::MidiCommandsReceived {
                commands,
                timestamp,
                peer,
            } = event
            {
                if let Ok((parsed_command, _)) = parse_midi_message(&commands) {
                    if let Some(mappings) = &mappings {
                        for mapping in mappings {
                            if mapping.matches_midi_command(&parsed_command) {
                                match parsed_command {
                                    MidiCommand::NoteOn { key, .. } => {
                                        info!("MIDI NoteOn {} matched a mapping.", key)
                                    }
                                    MidiCommand::ControlChange { control, value, .. } => {
                                        info!("MIDI CC {} ({}) matched a mapping.", control, value)
                                    }
                                    _ => (),
                                }
                                for action in &mapping.output {
                                    match action {
                                        MappingOutput::Wled(wled_action) => {
                                            if let Ok(payload) = serde_json::to_vec(&wled_action) {
                                                let _ = wled_sender.send(0, &payload);
                                            }
                                        } // utils::MappingOutput::Ddp(ddp_action) => {
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

    // Wait for all tasks to complete
    let _ = network_task.await;
    let _ = ddp_task.await;
    info!("Service has shut down gracefully.");
}

pub use rtp_midi_core::Config;
