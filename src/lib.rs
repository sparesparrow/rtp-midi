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

pub mod android;
pub mod ffi;
pub mod audio_input;
pub mod light_mapper;
pub mod ddp_output;
pub mod midi;

pub use midi::rtp::message::{MidiMessage, RtpMidiPacket};

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

pub mod midi_rtp_packet {
    use anyhow::{anyhow, Result};
    use bytes::{Buf, BufMut, Bytes, BytesMut};

    pub struct RtpMidiPacket {
        version: u8,
        padding: bool,
        extension: bool,
        csrc_count: u8,
        marker: bool,
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        
        journal_present: bool,
        
        midi_commands: Vec<super::MidiMessage>,
    }

    impl RtpMidiPacket {
        pub fn create(midi_messages: Vec<super::MidiMessage>) -> Self {
            Self {
                version: 2,
                padding: false,
                extension: false,
                csrc_count: 0,
                marker: true,
                payload_type: 97, // Dynamic Payload Type for MIDI
                sequence_number: 0,
                timestamp: 0,
                ssrc: rand::random::<u32>(), // Random SSRC
                journal_present: false,
                midi_commands: midi_messages,
            }
        }

        pub fn parse(data: &[u8]) -> Result<Self> {
            let mut reader = Bytes::from(data);
            if reader.len() < 12 {
                return Err(anyhow!("RTP header too short"));
            }

            let byte0 = reader.get_u8();
            let version = (byte0 >> 6) & 0x03;
            let padding = (byte0 >> 5) & 0x01 == 1;
            let extension = (byte0 >> 4) & 0x01 == 1;
            let csrc_count = byte0 & 0x0F;

            if version != 2 {
                return Err(anyhow!("Unsupported RTP version: {}", version));
            }

            let byte1 = reader.get_u8();
            let marker = (byte1 >> 7) & 0x01 == 1;
            let payload_type = byte1 & 0x7F;

            let sequence_number = reader.get_u16();
            let timestamp = reader.get_u32();
            let ssrc = reader.get_u32();

            // Skip CSRC identifiers if present
            for _ in 0..csrc_count {
                reader.get_u32();
            }

            // Parse MIDI journal if present
            let mut journal_present = false;
            if extension {
                // TODO: Parse RTP header extension
                warn!("RTP header extension not parsed.");
            }

            // Parse MIDI messages
            let mut midi_commands = Vec::new();
            while reader.has_remaining() {
                // Handle potential journal descriptor
                if reader.remaining() >= 1 && (reader.peek_u8() == 0x01 || reader.peek_u8() == 0x02) {
                    // Journal command (0x01 = MIDI Command, 0x02 = MIDI Command List)
                    let journal_descriptor = reader.get_u8();
                    let length = reader.get_u8(); // length of the journal entry (excluding itself)
                    if reader.remaining() < length as usize {
                        return Err(anyhow!("Incomplete MIDI journal entry"));
                    }
                    // Skip journal data for now
                    reader.advance(length as usize);
                    journal_present = true;
                    continue;
                }

                // Parse MIDI message delta time (Variable Length Quantity)
                let (delta_time, bytes_read) = parse_variable_length_quantity(reader.chunk())?;
                reader.advance(bytes_read);

                // Parse MIDI command
                if !reader.has_remaining() {
                    return Err(anyhow!("Incomplete MIDI command: Missing status byte"));
                }
                let status_byte = reader.get_u8();
                
                let mut command_bytes = vec![status_byte];

                let command_len = midi_command_length(status_byte);
                if reader.remaining() < command_len - 1 { // -1 because status byte is already read
                    return Err(anyhow!("Incomplete MIDI command: Expected {} data bytes, got {}", command_len - 1, reader.remaining()));
                }
                for _ in 0..(command_len - 1) {
                    command_bytes.push(reader.get_u8());
                }
                midi_commands.push(super::MidiMessage::new(delta_time, command_bytes));
            }

            Ok(Self {
                version,
                padding,
                extension,
                csrc_count,
                marker,
                payload_type,
                sequence_number,
                timestamp,
                ssrc,
                journal_present,
                midi_commands,
            })
        }

        pub fn serialize(&self) -> Result<Bytes> {
            let mut buf = BytesMut::with_capacity(12 + self.midi_commands.len() * 5); // Rough estimate

            let mut byte0 = (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4) | (self.csrc_count & 0x0F);
            buf.put_u8(byte0);

            let mut byte1 = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
            buf.put_u8(byte1);

            buf.put_u16(self.sequence_number);
            buf.put_u32(self.timestamp);
            buf.put_u32(self.ssrc);

            // TODO: Handle CSRC and Extension headers
            if self.extension {
                warn!("RTP header extension not serialized.");
            }

            for msg in &self.midi_commands {
                // Delta time
                let mut v_buf = [0u8; 4];
                let v_len = encode_variable_length_quantity(msg.delta_time, &mut v_buf)?;
                buf.put(&v_buf[..v_len]);

                // MIDI command
                buf.put(&msg.command[..]);
            }

            Ok(buf.freeze())
        }

        pub fn midi_commands(&self) -> &Vec<super::MidiMessage> { &self.midi_commands }
        pub fn set_sequence_number(&mut self, seq: u16) { self.sequence_number = seq; }
        pub fn set_ssrc(&mut self, ssrc: u32) { self.ssrc = ssrc; }
        pub fn set_journal_present(&mut self, present: bool) { self.journal_present = present; }
        pub fn sequence_number(&self) -> u16 { self.sequence_number }
        pub fn ssrc(&self) -> u32 { self.ssrc }
        pub fn journal_present(&self) -> bool { self.journal_present }
    }

    fn parse_variable_length_quantity(data: &[u8]) -> Result<(u32, usize)> {
        let mut value = 0u32;
        let mut bytes_read = 0;
        for &byte in data {
            bytes_read += 1;
            value = (value << 7) | (byte & 0x7F) as u32;
            if (byte & 0x80) == 0 {
                return Ok((value, bytes_read));
            }
            if bytes_read >= 4 { // Max 4 bytes for VLQ
                return Err(anyhow!("Variable Length Quantity too long or malformed"));
            }
        }
        Err(anyhow!("Incomplete Variable Length Quantity"))
    }

    fn encode_variable_length_quantity(value: u32, buf: &mut [u8; 4]) -> Result<usize> {
        if value == 0 {
            buf[0] = 0;
            return Ok(1);
        }
        let mut tmp = value;
        let mut len = 0;
        while tmp > 0 {
            buf[3 - len] = (tmp & 0x7F) as u8;
            if len < 3 { buf[3 - len] |= 0x80; }
            tmp >>= 7;
            len += 1;
        }
        buf.copy_within((4 - len)..4, 0);
        Ok(len)
    }

    // Helper to determine MIDI command length
    fn midi_command_length(status_byte: u8) -> usize {
        match status_byte & 0xF0 {
            0x80 => 3, // Note Off
            0x90 => 3, // Note On
            0xA0 => 3, // Polyphonic Key Pressure
            0xB0 => 3, // Control Change
            0xC0 => 2, // Program Change
            0xD0 => 2, // Channel Pressure
            0xE0 => 3, // Pitch Bend Change
            0xF0 => {
                match status_byte {
                    0xF0 => 1, // SysEx Start (length is variable, handled by subsequent bytes)
                    0xF1 => 2, // MIDI Time Code Quarter Frame
                    0xF2 => 3, // Song Position Pointer
                    0xF3 => 2, // Song Select
                    0xF6 => 1, // Tune Request
                    0xF8 => 1, // Timing Clock
                    0xFA => 1, // Start
                    0xFB => 1, // Continue
                    0xFC => 1, // Stop
                    0xFE => 1, // Active Sensing
                    0xFF => 1, // Reset
                    _ => 1, // Unknown system common/real-time message, assume 1 byte
                }
            }
            _ => 1, // Data byte or running status. This should not happen if status byte is correctly identified.
        }
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

pub mod midi {
    pub mod rtp {
        pub mod message;
    }
}

pub mod signaling_server {
    // Re-export contents of signaling_server_module under signaling_server
    pub use super::signaling_server_module::*;
}

pub mod wled_control;

pub fn run_service_loop(config: Config, running: Arc<AtomicBool>, rt_handle: tokio::runtime::Handle) {
    info!("Service loop starting...");

    let wled_ip = config.wled_ip.clone();
    let ddp_port = config.ddp_port.unwrap_or(4048);
    let led_count = config.led_count;
    let audio_device = config.audio_device.clone();
    let midi_port = config.midi_port.unwrap_or(5004);

    // Create channels for inter-thread communication
    let (audio_tx, audio_rx) = crossbeam_channel::unbounded();
    let (midi_tx, midi_rx) = crossbeam_channel::unbounded();

    // --- Audio Input Thread ---
    let running_clone = running.clone();
    let audio_input_config_name = audio_device.clone();
    let _audio_input_handle = thread::spawn(move || {
        info!("Audio input thread started.");
        let _audio_stream = match audio_input::start_audio_input(audio_input_config_name.as_deref(), audio_tx) {
            Ok(stream) => {
                info!("Audio stream started.");
                stream.play().expect("Failed to play audio stream");
                stream
            },
            Err(e) => {
                error!("Failed to start audio input: {}", e);
                running_clone.store(false, Ordering::SeqCst);
                return;
            }
        };
        while running_clone.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(100)); // Keep thread alive
        }
        info!("Audio input thread stopping.");
    });

    // --- Audio Processing & DDP Output Loop ---
    let mut prev_mags = Vec::new();
    let mut ddp_sender = rt_handle.block_on(async {
        ddp_output::create_ddp_sender(&wled_ip, ddp_port, led_count, false).expect("Failed to create DDP sender")
    });

    while running.load(Ordering::SeqCst) {
        // Process audio input
        if let Ok(audio_buffer) = audio_rx.try_recv() {
            let magnitudes = light_mapper::compute_fft_magnitudes(&audio_buffer, &mut prev_mags, 0.5);
            let leds = light_mapper::map_audio_to_leds(&magnitudes, led_count);
            if let Err(e) = rt_handle.block_on(async { ddp_output::send_ddp_frame(&mut ddp_sender, &leds) }) {
                error!("Failed to send DDP frame: {}", e);
            }
        }

        // TODO: Process MIDI input
        if let Ok(_midi_message) = midi_rx.try_recv() {
            // info!("Received MIDI message: {:?}", midi_message.command);
            // Placeholder for MIDI processing and WLED control based on MIDI
        }

        thread::sleep(Duration::from_millis(10)); // Control loop speed
    }

    info!("Service loop stopped.");
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
