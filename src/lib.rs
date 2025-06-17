use anyhow::Result;
use bytes::{Bytes, BytesMut};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Sample, SampleFormat};
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use num_traits;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerType {
    AudioServer,
    ClientApp,
}

/// Starts audio capture from the specified device (or default if None).
/// Sends audio buffers (Vec<f32>) to the provided channel sender.
pub fn start_audio_input(device_name: Option<&str>, tx: Sender<Vec<f32>>) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = if let Some(name) = device_name {
        host.input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| anyhow::anyhow!("Audio device not found: {}", name))?
    } else {
        host.default_input_device().ok_or_else(|| anyhow::anyhow!("No default audio input device"))?
    };
    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let config = config.into();
    let err_fn = |err| eprintln!("Audio input error: {}", err);
    let stream = match sample_format {
        SampleFormat::F32 => build_input_stream::<f32>(&device, &config, tx.clone(), err_fn)?,
        SampleFormat::I16 => build_input_stream::<i16>(&device, &config, tx.clone(), err_fn)?,
        SampleFormat::U16 => build_input_stream::<u16>(&device, &config, tx.clone(), err_fn)?,
        _ => todo!("Unsupported sample format"),
    };
    Ok(stream)
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: Sender<Vec<f32>>,
    err_fn: fn(cpal::StreamError),
) -> Result<cpal::Stream>
where
    T: Sample + cpal::SizedSample + num_traits::ToPrimitive + Send + 'static,
{
    let _channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut buffer = Vec::with_capacity(data.len());
            for &sample in data {
                buffer.push(num_traits::ToPrimitive::to_f32(&sample).unwrap_or(0.0));
            }
            // Optionally: downmix to mono or keep as is
            let _ = tx.send(buffer);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

/// Performs FFT on the input buffer and returns normalized magnitudes.
pub fn compute_fft_magnitudes(input: &[f32], prev: &mut Vec<f32>, smoothing: f32) -> Vec<f32> {
    let len = input.len().next_power_of_two();
    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(len);
    let mut buffer: Vec<rustfft::num_complex::Complex<f32>> = input.iter().map(|&x| rustfft::num_complex::Complex{ re: x, im: 0.0 }).collect();
    buffer.resize(len, rustfft::num_complex::Complex{ re: 0.0, im: 0.0 });
    fft.process(&mut buffer);
    let mut mags: Vec<f32> = buffer.iter().map(|c| c.norm()).collect();
    // Normalize
    let max = mags.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
    for m in mags.iter_mut() { *m /= max; }
    // Smoothing (simple moving average with previous frame)
    if prev.len() == mags.len() {
        for (m, p) in mags.iter_mut().zip(prev.iter()) {
            *m = smoothing * *p + (1.0 - smoothing) * *m;
        }
    }
    *prev = mags.clone();
    mags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_sine_wave() {
        // Generate a sine wave at 1/16th of the sample rate
        let n = 64;
        let freq_bin = 4;
        let mut input = vec![0.0f32; n];
        for i in 0..n {
            input[i] = (2.0 * PI * freq_bin as f32 * i as f32 / n as f32).sin();
        }
        let mut prev = vec![];
        let mags = compute_fft_magnitudes(&input, &mut prev, 0.0);
        // The magnitude should peak at bin 4 or n-4 (due to symmetry)
        let max_idx = mags.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert!(max_idx == freq_bin || max_idx == n - freq_bin, "Peak at wrong bin: {}", max_idx);
        // The peak should be much higher than the average
        let peak = mags[max_idx];
        let avg = mags.iter().sum::<f32>() / mags.len() as f32;
        assert!(peak > 3.0 * avg, "Peak not prominent enough");
    }

    #[test]
    fn test_fft_smoothing() {
        let n = 8;
        let input1 = vec![1.0; n];
        let input2 = vec![0.0; n];
        let mut prev = vec![0.0; n];
        let mags1 = compute_fft_magnitudes(&input1, &mut prev, 0.5);
        let mags2 = compute_fft_magnitudes(&input2, &mut prev, 0.5);
        // After smoothing, mags2 should be halfway between mags1 and 0
        for (m, m1) in mags2.iter().zip(mags1.iter()) {
            assert!((*m - m1 * 0.5).abs() < 1e-3, "Smoothing failed");
        }
    }
}

use std::fs;
use std::path::Path;

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
mod tests {
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

/// Maps FFT magnitudes to LED RGB values.
/// Returns a Vec<u8> of length led_count * 3 (RGB for each LED).
pub fn map_audio_to_leds(magnitudes: &[f32], led_count: usize) -> Vec<u8> {
    let mut leds = vec![0u8; led_count * 3];
    let band_size = magnitudes.len() / 3;
    let bass = magnitudes.iter().take(band_size).cloned().fold(0.0, f32::max);
    let mid = magnitudes.iter().skip(band_size).take(band_size).cloned().fold(0.0, f32::max);
    let treble = magnitudes.iter().skip(2 * band_size).cloned().fold(0.0, f32::max);
    for i in 0..led_count {
        leds[i * 3] = (bass * 255.0) as u8;   // Red
        leds[i * 3 + 1] = (mid * 255.0) as u8; // Green
        leds[i * 3 + 2] = (treble * 255.0) as u8; // Blue
    }
    leds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_audio_to_leds_bass() {
        let mags = vec![1.0, 0.0, 0.0]; // Only bass
        let leds = map_audio_to_leds(&mags, 2);
        assert_eq!(leds, vec![255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_mid() {
        let mags = vec![0.0, 1.0, 0.0]; // Only mid
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 255, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_treble() {
        let mags = vec![0.0, 0.0, 1.0]; // Only treble
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 0, 255]);
    }
}

/// Sends a frame of LED data to the DDP receiver (WLED).
pub fn send_ddp_frame(
    sender: &mut ddp_rs::connection::DDPConnection,
    data: &[u8],
) -> Result<()> {
    sender.write(data)?;
    Ok(())
}

/// Creates a DDPConnection for the given target IP, port, and pixel config.
/// Note: ddp-rs 0.3 only supports RGB (3 channels per pixel) via PixelConfig::default().
/// RGBW is not directly supported in this version of the crate.
pub fn create_ddp_sender(
    ip: &str,
    port: u16,
    _led_count: usize,
    _rgbw: bool,
) -> Result<ddp_rs::connection::DDPConnection> {
    // ddp-rs 0.3 does not support RGBW configuration. Only RGB is supported.
    // If you need RGBW, you must use a different crate or fork ddp-rs.
    let pixel_config = ddp_rs::protocol::PixelConfig::default(); // Always RGB
    let addr = format!("{}:{}", ip, port);
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    let sender = ddp_rs::connection::DDPConnection::try_new(addr, pixel_config, ddp_rs::protocol::ID::Custom(1), socket)?;
    Ok(sender)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[test]
    fn test_create_ddp_sender_invalid_addr() {
        let res = create_ddp_sender("256.256.256.256", 4048, 10, false);
        assert!(res.is_err(), "Should fail for invalid IP");
    }

    // Note: For a real integration test, bind a UDP socket and check for received data.
}

// Placeholder for MidiMessage struct, as src/midi/message.rs was not found
// This should be replaced with the actual MidiMessage definition if it exists elsewhere
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiMessage {
    command_byte: u8,
    data_bytes: Vec<u8>,
}

impl MidiMessage {
    pub fn command(&self) -> u8 {
        self.command_byte
    }
}

pub mod midi_rtp_packet {
    use super::{Bytes, BytesMut, MidiMessage, Result, Serialize, Deserialize, SystemTime, UNIX_EPOCH}; 

    const RTP_VERSION: u8 = 2;
    const MIDI_PAYLOAD_TYPE: u8 = 97; // Standardní číslo pro MIDI

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RtpMidiPacket {
        // RTP Header
        version: u8,
        padding: bool,
        extension: bool,
        csrc_count: u8,
        marker: bool,
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        
        // RTP MIDI specifické položky
        journal_present: bool,
        first_midi_command: u8,
        
        // MIDI data
        midi_commands: Vec<MidiMessage>,
    }

    impl RtpMidiPacket {
        /// Vytvoří nový RTP-MIDI paket s danými MIDI zprávami
        pub fn create(midi_messages: Vec<MidiMessage>) -> Self {
            // Získání aktuálního času v milisekundách od epochy
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Čas před UNIX epochou")
                .as_millis() as u32;
            
            let first_midi_command = if midi_messages.is_empty() { 0 } else { midi_messages[0].command() };
            
            Self {
                version: RTP_VERSION,
                padding: false,
                extension: false,
                csrc_count: 0,
                marker: false,
                payload_type: MIDI_PAYLOAD_TYPE,
                sequence_number: 0, // Bude nastaveno sessionem
                timestamp,
                ssrc: 0, // Bude nastaveno sessionem
                journal_present: false,
                first_midi_command,
                midi_commands: midi_messages,
            }
        }
        
        /// Parsuje RTP-MIDI paket z bytů
        pub fn parse(data: &[u8]) -> Result<Self> {
            if data.len() < 12 { // Minimum size for RTP header (12 bytes)
                return Err(anyhow::anyhow!("Data too short for RTP header"));
            }
            
            // RTP Header
            let b0 = data[0];
            let b1 = data[1];
            
            let version = (b0 >> 6) & 0x03;
            let padding = ((b0 >> 5) & 0x01) != 0;
            let extension = ((b0 >> 4) & 0x01) != 0;
            let csrc_count = b0 & 0x0F;
            let marker = (b1 >> 7) != 0;
            let payload_type = b1 & 0x7F;
            
            let sequence_number = u16::from_be_bytes([data[2], data[3]]);
            let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            
            let header_size = 12 + (csrc_count as usize) * 4 + if extension { 4 } else { 0 }; // Basic RTP header + CSRC + Extension header (simplified)
            
            if data.len() < header_size + 1 { // Need at least 1 byte for MIDI header
                 return Err(anyhow::anyhow!("Data too short for MIDI header"));
            }
            
            // RTP MIDI Header (1 byte)
            let midi_header = data[header_size];
            let journal_present = ((midi_header >> 7) & 0x01) != 0;
            let first_midi_command = midi_header & 0x7F;
            
            // Parsování MIDI zpráv
            let _midi_data = &data[header_size + 1..]; // Changed to _midi_data
            let midi_commands = Vec::new(); // Removed mut
            
            // TODO: Implement proper MIDI message parsing according to RFC 6295 / RFC 4695
            // This requires handling delta times, MIDI commands, and data bytes.
            // For now, we'll just store the raw data after the MIDI header as a placeholder.
            // This is incorrect for actual RTP-MIDI.
            // For a proper implementation, refer to RFC 6295 section 3.2.
            
            // Placeholder: Copying raw MIDI data bytes (incorrect for RTP-MIDI structure)
            // midi_commands = midi_data.to_vec(); // This is not MidiMessage objects
            
            // A proper implementation would loop through midi_data, parse each MIDI command
            // including its delta time, and construct MidiMessage objects.
            
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
                first_midi_command,
                midi_commands, // This will be empty or incorrectly populated with raw bytes
            })
        }
        
        /// Serializuje RTP-MIDI paket do bytů
        pub fn serialize(&self) -> Result<Bytes> {
            let mut buffer = BytesMut::with_capacity(1024);
            
            // RTP header
            let b0 = (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4) | (self.csrc_count & 0x0F);
            let b1 = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
            buffer.extend_from_slice(&[b0, b1]);
            buffer.extend_from_slice(&self.sequence_number.to_be_bytes());
            buffer.extend_from_slice(&self.timestamp.to_be_bytes());
            buffer.extend_from_slice(&self.ssrc.to_be_bytes());
            
            // RTP MIDI Header (1 byte)
            let midi_header = ((self.journal_present as u8) << 7) | (self.first_midi_command & 0x7F);
            buffer.extend_from_slice(&[midi_header]);
            
            // MIDI data
            // TODO: Implement proper serialization of MidiMessage objects into RTP-MIDI format
            // This requires encoding delta times and MIDI command bytes according to RFC 6295 section 3.3.
            // For now, we'll just append raw command bytes if available (incorrect).
            
            // Placeholder: Appending raw command bytes (incorrect)
            for cmd in &self.midi_commands {
                // This is incorrect; need to serialize the full MidiMessage including delta time
                // and handle running status.
                buffer.extend_from_slice(&[cmd.command()]); // Appending just the command byte
                // Need to also append data bytes depending on the command.
            }
            
            Ok(buffer.freeze())
        }
        
        /// Vrací odkaz na MIDI příkazy v paketu
        pub fn midi_commands(&self) -> Option<&Vec<MidiMessage>> {
            if self.midi_commands.is_empty() {
                None
            } else {
                Some(&self.midi_commands)
            }
        }
        
        /// Nastaví číslo sekvence
        pub fn set_sequence_number(&mut self, seq: u16) {
            self.sequence_number = seq;
        }
        
        /// Nastaví SSRC identifikátor
        pub fn set_ssrc(&mut self, ssrc: u32) {
            self.ssrc = ssrc;
        }
        
        /// Nastaví příznak přítomnosti žurnálu
        pub fn set_journal_present(&mut self, present: bool) {
            self.journal_present = present;
        }
    }
}

pub mod signaling_server_module {
    use super::{HashMap, Arc, Mutex, TcpListener, TcpStream, mpsc, accept_async, Message, SinkExt, StreamExt, info, warn, error};
    use serde_json::json; 
    use serde::{Deserialize, Serialize};
    
    #[derive(Serialize, Deserialize, Debug)]
    pub struct RegisterPayload {
        pub peer_type: super::PeerType,
        pub client_id: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SignalingMessage {
        pub message_type: String,
        pub sender_id: String,
        pub receiver_id: Option<String>,
        pub payload: serde_json::Value,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ClientNotification {
        pub client_id: String,
        pub peer_type: super::PeerType,
    }

    #[derive(Clone)]
    pub struct Clients {
        peers: Arc<Mutex<HashMap<String, (super::PeerType, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)>>>,
    }

    impl Clients {
        pub fn new() -> Self {
            Self {
                peers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn register(&self, client_id: String, peer_type: super::PeerType, tx: mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
            let mut peers = self.peers.lock().unwrap();
            peers.insert(client_id, (peer_type, tx));
        }

        pub fn unregister(&self, client_id: &str) {
            let mut peers = self.peers.lock().unwrap();
            peers.remove(client_id);
        }

        pub fn get_audio_server(&self) -> Option<(String, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)> {
            let peers = self.peers.lock().unwrap();
            for (id, (peer_type, tx)) in peers.iter() {
                if let super::PeerType::AudioServer = peer_type {
                    return Some((id.clone(), tx.clone()));
                }
            }
            None
        }

        pub fn get_peer(&self, client_id: &str) -> Option<mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>> {
            let peers = self.peers.lock().unwrap();
            peers.get(client_id).map(|(_, tx)| tx.clone())
        }

        pub fn get_all_clients(&self) -> Vec<(String, super::PeerType)> {
            let peers = self.peers.lock().unwrap();
            peers.iter().map(|(id, (peer_type, _))| (id.clone(), peer_type.clone())).collect()
        }
    }

    pub async fn handle_connection(clients: Clients, stream: TcpStream) {
        info!("New connection from {}", stream.peer_addr().unwrap());
        
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("Error during WebSocket handshake: {}", e);
                return;
            }
        };
        
        let (mut ws_write, mut ws_read) = ws_stream.split();
        
        let (tx, mut rx) = mpsc::channel::<Result<Message, tokio_tungstenite::tungstenite::Error>>(64);
        
        let mut client_id = String::new();
        let mut _peer_type = None;
        
        // Listen for incoming messages
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_write.send(msg.unwrap()).await {
                    error!("Error sending message over WebSocket: {}", e);
                    break;
                }
            }
        });
        
        while let Some(msg) = ws_read.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Error reading message from WebSocket: {}", e);
                    break;
                }
            };
            
            if msg.is_close() {
                info!("WebSocket connection closed by peer");
                break;
            }
            
            if let Message::Text(text) = msg {
                match serde_json::from_str::<SignalingMessage>(&text) {
                    Ok(signaling_msg) => {
                        match signaling_msg.message_type.as_str() {
                            "register" => {
                                if let Ok(register_payload) = serde_json::from_value::<RegisterPayload>(signaling_msg.payload) {
                                    client_id = register_payload.client_id.clone();
                                    _peer_type = Some(register_payload.peer_type.clone());
                                    
                                    clients.register(client_id.clone(), register_payload.peer_type, tx.clone());
                                    info!("Registered client: {} (type: {:?})", client_id, _peer_type);
                                    
                                    // Inform client of successful registration
                                    let response = SignalingMessage {
                                        message_type: "register_success".to_string(),
                                        sender_id: "server".to_string(),
                                        receiver_id: Some(client_id.clone()),
                                        payload: json!({
                                            "message": "Successfully registered",
                                            "registered_id": client_id,
                                            "clients": clients.get_all_clients()
                                        }),
                                    };
                                    
                                    if let Ok(response_str) = serde_json::to_string(&response) {
                                        let _ = tx.send(Ok(Message::text(response_str))).await;
                                    } else {
                                        error!("Failed to serialize register_success response");
                                    }
                                    
                                    // Inform audio server about the new client
                                    if let Some(super::PeerType::ClientApp) = _peer_type {
                                        notify_audio_server_of_new_client(&clients, &client_id).await;
                                    }
                                } else {
                                    warn!("Failed to parse register payload from client {}", client_id);
                                }
                            },
                            "list_peers" => {
                                let current_clients = clients.get_all_clients();
                                let response = SignalingMessage {
                                    message_type: "peer_list".to_string(),
                                    sender_id: "server".to_string(),
                                    receiver_id: Some(client_id.clone()),
                                    payload: json!({
                                        "peers": current_clients
                                    }),
                                };
                                 
                                if let Ok(response_str) = serde_json::to_string(&response) {
                                    if tx.send(Ok(Message::text(response_str))).await.is_err() {
                                        error!("Failed to send peer_list to client {}", client_id);
                                    }
                                } else {
                                    error!("Failed to serialize peer_list response for client {}", client_id);
                                }
                            },
                            "offer" | "answer" | "ice_candidate" => {
                                if let Some(receiver_id) = &signaling_msg.receiver_id {
                                    if let Some(peer_tx) = clients.get_peer(receiver_id) {
                                        if let Ok(msg_str) = serde_json::to_string(&signaling_msg) {
                                            if let Err(e) = peer_tx.send(Ok(Message::text(msg_str))).await {
                                                error!("Error forwarding message to {}: {}", receiver_id, e);
                                            }
                                        } else {
                                            error!("Failed to serialize signaling message for forwarding to {}", receiver_id);
                                        }
                                    } else {
                                        warn!("Target peer {} not found for message type {}", receiver_id, signaling_msg.message_type);
                                    }
                                } else {
                                    warn!("Receiver ID missing for message type {}", signaling_msg.message_type);
                                }
                            },
                            _ => {
                                warn!("Unknown message type received: {}", signaling_msg.message_type);
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to parse signaling message: {}", e);
                    }
                }
            } else if msg.is_binary() {
                warn!("Received binary message, expected text");
            }
        }
        
        if !client_id.is_empty() {
            info!("Client {} disconnected", client_id);
            clients.unregister(&client_id);
        }
    }

    async fn notify_audio_server_of_new_client(clients: &Clients, client_id: &str) {
        if let Some((server_id, server_tx)) = clients.get_audio_server() {
            let notification_payload = SignalingMessage {
                message_type: "new_client".to_string(),
                sender_id: "server".to_string(),
                receiver_id: Some(server_id.clone()),
                payload: json!({
                    "client_id": client_id
                }),
            };
            
            if let Ok(msg_str) = serde_json::to_string(&notification_payload) {
                if server_tx.send(Ok(Message::text(msg_str))).await.is_err() {
                    error!("Failed to notify audio server {} about client {}", server_id, client_id);
                }
            } else {
                error!("Failed to serialize new_client notification for audio server {}", server_id);
            }
        } else {
            info!("No audio server registered to notify about new client {}", client_id);
        }
    }

    pub async fn run_server(listener: TcpListener) -> anyhow::Result<()> {
        info!("Signaling server running on {}", listener.local_addr()?);

        let clients = Clients::new();

        while let Ok((stream, _)) = listener.accept().await {
            let peer_addr = stream.peer_addr()?;
            info!("Accepted connection from: {}", peer_addr);
            
            let clients_clone = clients.clone();
            tokio::spawn(async move {
                handle_connection(clients_clone, stream).await;
            });
        }

        Ok(())
    }
}

// Re-export modules as rtp_midi::midi and rtp_midi::signaling_server
pub mod midi {
    pub mod rtp {
        pub use super::super::midi_rtp_packet::*;
    }
    pub mod message {
        // Placeholder for midi::message module, if it existed
        pub use super::super::MidiMessage;
    }
}
pub mod signaling_server {
    pub use super::signaling_server_module::*;
} 