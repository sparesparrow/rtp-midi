use anyhow::Result;
use bytes::{Bytes, BytesMut};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use num_traits;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_remote::TrackRemote;

pub mod android;
pub mod ffi;
pub mod audio_input;
pub mod light_mapper;
pub mod ddp_output;
pub mod midi;
pub mod mapping;

pub use midi::rtp::message::{MidiMessage, RtpMidiPacket, MidiCommand};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerType {
    AudioServer,
    ClientApp,
}

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

pub mod signaling_server_module {
    use anyhow::Result;
    use futures_util::{SinkExt, StreamExt};
    use log::{error, info, warn};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct RegisterPayload {
        pub peer_type: super::PeerType,
        pub client_id: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type")]
    pub enum SignalingMessage {
        Register(RegisterPayload),
        ClientList {
            clients: Vec<ClientNotification>,
        },
        // Add other message types as needed
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ClientNotification {
        pub client_id: String,
        pub peer_type: super::PeerType,
    }

    #[derive(Clone)]
    pub struct Clients {
        peers: Arc<Mutex<HashMap<String, (super::PeerType, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)>>>,
    }

    impl Clients {
        pub fn new() -> Self { Self { peers: Arc::new(Mutex::new(HashMap::new())), } }

        pub fn register(&self, client_id: String, peer_type: super::PeerType, tx: mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
            info!("Registering client: {} ({:?})", client_id, peer_type);
            self.peers.lock().unwrap().insert(client_id, (peer_type, tx));
        }

        pub fn unregister(&self, client_id: &str) {
            info!("Unregistering client: {}", client_id);
            self.peers.lock().unwrap().remove(client_id);
        }

        pub fn get_audio_server(&self) -> Option<(String, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)> {
            self.peers.lock().unwrap().iter().find_map(|(id, (peer_type, tx))| {
                if *peer_type == super::PeerType::AudioServer {
                    Some((id.clone(), tx.clone()))
                } else {
                    None
                }
            })
        }

        pub fn get_peer(&self, client_id: &str) -> Option<mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>> { self.peers.lock().unwrap().get(client_id).map(|(_, tx)| tx.clone()) }

        pub fn get_all_clients(&self) -> Vec<(String, super::PeerType)> {
            self.peers.lock().unwrap().iter().map(|(id, (peer_type, _))| (id.clone(), peer_type.clone())).collect()
        }
    }

    pub async fn handle_connection(clients: Clients, stream: TcpStream) {
        let peer_addr = match stream.peer_addr() {
            Ok(addr) => addr.to_string(),
            Err(_) => "unknown".to_string(),
        };
        info!("New WebSocket connection from: {}", peer_addr);

        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("Error during websocket handshake with {}: {}", peer_addr, e);
                return;
            }
        };
        info!("WebSocket handshake successful with {}", peer_addr);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<Result<Message, tokio_tungstenite::tungstenite::Error>>(100);

        let client_id = Uuid::new_v4().to_string();
        let mut peer_type_opt = None;

        // Spawn a task to send messages from the mpsc channel to the websocket
        let ws_sender_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_sender.send(msg?).await {
                    error!("Failed to send message over websocket: {}", e);
                    break;
                }
            }
            info!("WebSocket sender task for {} terminated.", peer_addr);
        });

        // Main loop for receiving messages from the websocket
        while let Some(msg_res) = ws_receiver.next().await {
            let msg = match msg_res {
                Ok(m) => m,
                Err(e) => {
                    error!("Error receiving message from {}: {}", peer_addr, e);
                    break;
                }
            };

            if msg.is_text() || msg.is_binary() {
                let text = msg.to_text().unwrap_or("invalid utf8");
                info!("Received message from {}: {}", peer_addr, text);

                let signaling_message: Result<SignalingMessage, _> = serde_json::from_str(text);
                match signaling_message {
                    Ok(SignalingMessage::Register(payload)) => {
                        info!("Client {} registered as {:?}", payload.client_id, payload.peer_type);
                        peer_type_opt = Some(payload.peer_type.clone());
                        clients.register(payload.client_id.clone(), payload.peer_type, tx.clone());

                        // Notify newly connected client about existing clients
                        let all_clients = clients.get_all_clients();
                        let client_notifications: Vec<ClientNotification> = all_clients.into_iter().map(|(id, p_type)| ClientNotification { client_id: id, peer_type: p_type }).collect();
                        if let Ok(msg_text) = serde_json::to_string(&SignalingMessage::ClientList { clients: client_notifications }) {
                            if let Err(e) = tx.send(Ok(Message::Text(msg_text))).await {
                                error!("Failed to send client list to new client: {}", e);
                            }
                        }

                        // Notify audio server about new client
                        if payload.peer_type != super::PeerType::AudioServer {
                            notify_audio_server_of_new_client(&clients, &payload.client_id).await;
                        }
                    },
                    // Handle other signaling message types
                    _ => warn!("Unhandled signaling message type or malformed message: {}", text),
                }
            } else if msg.is_close() {
                info!("Connection closed by client: {}", peer_addr);
                break;
            }
        }

        clients.unregister(&client_id);
        info!("Client {} ({:?}) disconnected.", client_id, peer_type_opt.unwrap_or(super::PeerType::ClientApp));
        ws_sender_task.abort(); // Ensure sender task is cleaned up
    }

    async fn notify_audio_server_of_new_client(clients: &Clients, client_id: &str) {
        if let Some((audio_server_id, audio_server_tx)) = clients.get_audio_server() {
            info!("Notifying audio server {} about new client {}", audio_server_id, client_id);
            let notification = ClientNotification {
                client_id: client_id.to_string(),
                peer_type: super::PeerType::ClientApp, // Assuming new clients are ClientApp for now
            };
            let msg = SignalingMessage::ClientList { clients: vec![notification] };
            if let Ok(msg_text) = serde_json::to_string(&msg) {
                if let Err(e) = audio_server_tx.send(Ok(Message::Text(msg_text))).await {
                    error!("Failed to notify audio server: {}", e);
                }
            }
        }
    }

    pub async fn run_server(listener: TcpListener) -> anyhow::Result<()> {
        info!("Signaling server listening on {}", listener.local_addr()?);
        let clients = Clients::new();

        loop {
            let (stream, _) = listener.accept().await?;
            let clients_clone = clients.clone();
            tokio::spawn(async move { handle_connection(clients_clone, stream).await; });
        }
    }
}

pub mod signaling_server {
    // Re-export contents of signaling_server_module under signaling_server
    pub use super::signaling_server_module::*;
}

pub mod wled_control;

pub async fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let audio_device_name = config.audio_device.clone();
    let mappings = config.mappings.clone();

    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();
    let (midi_tx, midi_rx) = crossbeam_channel::unbounded();

    // Audio Input Thread
    let audio_input_config = config.clone();
    let audio_input_handle = std::thread::spawn(move || {
        audio_input::start_audio_input(audio_input_config, audio_tx)
            .expect("Failed to start audio input stream");
    });

    // --- WebRTC/Signaling Setup ---
    let my_client_id = format!("rtp-midi-service-{}", Uuid::new_v4());
    let signaling_url = "ws://127.0.0.1:8080/signaling".to_string(); // Assuming signaling server runs on localhost:8080

    let (ws_stream, _) = connect_async(signaling_url)
        .await
        .expect("Failed to connect to signaling server");
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let ws_write_arc = Arc::new(tokio::sync::Mutex::new(ws_write));

    // Register with signaling server
    let register_msg = SignalingMessage {
        message_type: "register".to_string(),
        sender_id: my_client_id.clone(),
        receiver_id: Some("server".to_string()),
        payload: serde_json::json!({
            "peer_type": PeerType::AudioServer,
            "client_id": my_client_id.clone(),
        }),
    };
    ws_write_arc.lock().await.send(Message::Text(serde_json::to_string(&register_msg).unwrap())).await.expect("Failed to send register message");
    info!("Registered with signaling server as {}", my_client_id);

    // WebRTC Peer Connection setup
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().unwrap();
    let api = APIBuilder::new().with_media_engine(media_engine).build();
    let mut rtc_config = RTCConfiguration::default();
    rtc_config.ice_servers = vec![
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            ..Default::default()
        },
    ];
    let peer_connection = Arc::new(api.new_peer_connection(rtc_config).await.unwrap());

    let peer_connection_clone = Arc::clone(&peer_connection);
    let my_client_id_clone = my_client_id.clone();
    let midi_tx_clone_for_dc = midi_tx.clone();

    // Handle incoming data channels (for MIDI)
    peer_connection.on_data_channel(Box::new(move |data_channel: Arc<RTCDataChannel>| {
        let dc_clone = Arc::clone(&data_channel);
        let midi_tx_clone_inner = midi_tx_clone_for_dc.clone();
        Box::pin(async move {
            info!("Received data channel: {}", data_channel.label());
            if data_channel.label() == "midi" {
                data_channel.on_message(Box::new(move |msg: Bytes| {
                    let midi_tx = midi_tx_clone_inner.clone();
                    Box::pin(async move {
                        if msg.is_empty() {
                            return;
                        }
                        info!("MIDI data received over WebRTC data channel: {:?}", msg);
                        // Parse raw MIDI bytes using the new parser
                        let mut cursor = 0;
                        while cursor < msg.len() {
                            match midi::parser::parse_midi_message(&msg[cursor..]) {
                                Ok((command, bytes_read)) => {
                                    info!("Parsed MIDI command: {:?}", command);
                                    if let Err(e) = midi_tx.send(command) {
                                        error!("Failed to send MIDI command to channel: {}", e);
                                    }
                                    cursor += bytes_read;
                                },
                                Err(e) => {
                                    error!("Failed to parse MIDI message from data channel at {}: {}", cursor, e);
                                    break; // Stop parsing if an error occurs
                                }
                            }
                        }
                    })
                }));
            }
        })
    })).await;

    // Handle ICE candidates
    let ws_write_ice_clone = Arc::clone(&ws_write_arc);
    let my_client_id_ice_clone = my_client_id.clone();
    peer_connection.on_ice_candidate(Box::new(move |candidate| {
        let ws_write = ws_write_ice_clone.clone();
        let my_client_id = my_client_id_ice_clone.clone();
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_msg = SignalingMessage {
                    message_type: "ice_candidate".to_string(),
                    sender_id: my_client_id,
                    receiver_id: None,
                    payload: serde_json::json!({
                        "candidate": candidate.to_json().unwrap(),
                        "sdpMid": candidate.sdp_mid,
                        "sdpMLineIndex": candidate.sdp_mline_index,
                    }),
                };
                if let Ok(msg_str) = serde_json::to_string(&candidate_msg) {
                    if let Err(e) = ws_write.lock().await.send(Message::text(msg_str)).await {
                        error!("Failed to send ICE candidate: {}", e);
                    } else {
                        info!("Sent ICE candidate to signaling server.");
                    }
                }
            }
        })
    })).await;

    // Handle peer connection state changes
    peer_connection.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
        Box::pin(async move {
            info!("WebRTC connection state changed: {:?}", state);
            if state == RTCPeerConnectionState::Failed ||
               state == RTCPeerConnectionState::Closed ||
               state == RTCPeerConnectionState::Disconnected {
                error!("WebRTC connection terminated. Attempting to restart or handle gracefully.");
                // TODO: Implement reconnection logic or graceful shutdown
            }
        })
    })).await;

    // Task to read messages from signaling server
    let peer_connection_signaling_clone = Arc::clone(&peer_connection);
    let my_client_id_signaling_clone = my_client_id.clone();
    let ws_write_signaling_clone = Arc::clone(&ws_write_arc);

    tokio::spawn(async move {
        while let Some(msg_res) = ws_read.next().await {
            let msg = match msg_res {
                Ok(m) => m,
                Err(e) => {
                    error!("Error receiving message from signaling server: {}", e);
                    break;
                }
            };

            if msg.is_text() {
                let text = msg.to_text().unwrap();
                let signaling_msg: SignalingMessage = match serde_json::from_str(text) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to parse signaling message: {}. Message: {}", e, text);
                        continue;
                    }
                };
                info!("Received signaling message: {:?}", signaling_msg);

                match signaling_msg.message_type.as_str() {
                    "offer" => {
                        let offer_sdp = signaling_msg.payload.get("sdp").and_then(|v| v.as_str()).unwrap_or("");
                        let offer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(offer_sdp.to_string()).unwrap();
                        if let Err(e) = peer_connection_signaling_clone.set_remote_description(offer).await {
                            error!("Failed to set remote description (offer): {}", e);
                            continue;
                        }

                        let answer = peer_connection_signaling_clone.create_answer(None).await.unwrap();
                        if let Err(e) = peer_connection_signaling_clone.set_local_description(answer.clone()).await {
                            error!("Failed to set local description (answer): {}", e);
                            continue;
                        }

                        let answer_msg = SignalingMessage {
                            message_type: "answer".to_string(),
                            sender_id: my_client_id_signaling_clone.clone(),
                            receiver_id: Some(signaling_msg.sender_id.clone()),
                            payload: serde_json::json!({
                                "sdp": peer_connection_signaling_clone.local_description().await.unwrap().sdp,
                                "type": "answer"
                            }),
                        };
                        let msg_str = serde_json::to_string(&answer_msg).unwrap();
                        if let Err(e) = ws_write_signaling_clone.lock().await.send(Message::text(msg_str)).await {
                            error!("Failed to send answer: {}", e);
                        }
                        info!("Sent answer to {}.", signaling_msg.sender_id);
                    },
                    "answer" => {
                        let answer_sdp = signaling_msg.payload.get("sdp").and_then(|v| v.as_str()).unwrap_or("");
                        let answer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::answer(answer_sdp.to_string()).unwrap();
                        if let Err(e) = peer_connection_signaling_clone.set_remote_description(answer).await {
                            error!("Failed to set remote description (answer): {}", e);
                        }
                        info!("Received answer from {}.", signaling_msg.sender_id);
                    },
                    "ice_candidate" => {
                        let candidate_payload = signaling_msg.payload.get("candidate").unwrap();
                        if let Ok(candidate_init) = serde_json::from_value::<RTCIceCandidateInit>(candidate_payload.clone()) {
                            if let Err(e) = peer_connection_signaling_clone.add_ice_candidate(candidate_init).await {
                                eprintln!("[ServiceLoop] Chyba při přidávání ICE kandidáta: {}", e);
                            } else {
                                info!("Added ICE candidate from {}.", signaling_msg.sender_id);
                            }
                        } else {
                            eprintln!("[ServiceLoop] Nepodařilo se deserializovat ICE kandidáta z payloadu.");
                        }
                    },
                    "new_client_list" | "peer_list" | "register_success" => {
                        info!("Received client list or register success: {:?}", signaling_msg.payload);
                        // Optionally handle peer list updates here if needed for direct connection management
                    }
                    _ => info!("Unhandled signaling message type: {}", signaling_msg.message_type),
                }
            }
        }
    });

    // Main service loop for audio processing and DDP output
    let mut prev_mags = Vec::new();
    let mut ddp_sender = ddp_output::create_ddp_sender(
        &wled_ip,
        config.ddp_port.unwrap_or(4048),
        config.led_count,
        false // is_rgbw
    ).expect("Failed to create DDP sender");

    while running.load(Ordering::SeqCst) {
        // Process audio data
        if let Ok(audio_buffer) = audio_rx.try_recv() {
            let magnitudes = light_mapper::compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);
            let leds = light_mapper::map_audio_to_leds(&magnitudes, config.led_count);
            if let Err(e) = ddp_output::send_ddp_frame(&mut ddp_sender, &leds) {
                error!("Failed to send DDP frame: {}", e);
            }
        }

        // Process MIDI data
        while let Ok(midi_command) = midi_rx.try_recv() {
            info!("Processing MIDI command: {:?}", midi_command);
            // Example: set WLED preset based on MIDI Note On
            if let MidiCommand::NoteOn { key, .. } = midi_command {
                // Map MIDI key to a WLED preset (e.g., C3=48 -> Preset 1)
                let preset_id = (key as i32 - 47).max(1);
                // Ensure preset_id is at least 1
                info!("Attempting to set WLED preset {} from MIDI note {}", preset_id, key);
                if let Err(e) = wled_control::set_wled_preset(&wled_ip, preset_id).await {
                    error!("Failed to set WLED preset: {}", e);
                }
            } else {
                match midi_command {
                    MidiCommand::NoteOn { channel, key, velocity } => {
                        info!("Note On: ch={}, key={}, vel={}", channel, key, velocity);
                        if let Some(mappings) = &mappings {
                            for mapping in mappings {
                                if mapping.matches_midi_command(&MidiCommand::NoteOn { channel, key, velocity }) {
                                    info!("MIDI Note On matched a mapping, executing WLED actions.");
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
                    },
                    MidiCommand::ControlChange { channel, control, value } => {
                        info!("Control Change: ch={}, ctrl={}, val={}", channel, control, value);
                        if let Some(mappings) = &mappings {
                            for mapping in mappings {
                                if mapping.matches_midi_command(&MidiCommand::ControlChange { channel, control, value }) {
                                    info!("MIDI Control Change matched a mapping, executing WLED actions.");
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
                                                    error!("Failed to set WLED color for CC: R={}, G={}, B={}: {}", r, g, b, e);
                                                }
                                            },
                                            mapping::WledOutputAction::SetEffect { id, speed, intensity } => {
                                                if let Err(e) = wled_control::set_wled_effect(&wled_ip, *id, *speed, *intensity).await {
                                                    error!("Failed to set WLED effect for CC: {}: {}", id, e);
                                                }
                                            },
                                            mapping::WledOutputAction::SetPalette { id } => {
                                                if let Err(e) = wled_control::set_wled_palette(&wled_ip, *id).await {
                                                    error!("Failed to set WLED palette for CC: {}: {}", id, e);
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    },
                    _ => {}
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
