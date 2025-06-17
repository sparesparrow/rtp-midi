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

/// Maps FFT magnitudes to LED RGB values.
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
mod mapping_tests {
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
pub fn send_ddp_frame(sender: &mut ddp_rs::connection::DDPConnection, data: &[u8]) -> Result<()> {
    sender.write(data)?;
    Ok(())
}

pub fn create_ddp_sender(ip: &str, port: u16, _led_count: usize, _rgbw: bool) -> Result<ddp_rs::connection::DDPConnection> {
    let pixel_config = ddp_rs::protocol::PixelConfig::default(); // Always RGB
    let addr = format!("{}:{}", ip, port);
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    let sender = ddp_rs::connection::DDPConnection::try_new(addr, pixel_config, ddp_rs::protocol::ID::Custom(1), socket)?;
    Ok(sender)
}

#[cfg(test)]
mod ddp_tests {
    use super::*;

    #[test]
    fn test_create_ddp_sender_invalid_addr() {
        let res = create_ddp_sender("256.256.256.256", 4048, 10, false);
        assert!(res.is_err(), "Should fail for invalid IP");
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiMessage {
    pub delta_time: u32,
    pub command: Vec<u8>,
}

impl MidiMessage {
    pub fn new(delta_time: u32, command: Vec<u8>) -> Self {
        Self { delta_time, command }
    }
}

pub mod midi_rtp_packet {
    use super::{Bytes, BytesMut, MidiMessage, Result, Serialize, Deserialize, SystemTime, UNIX_EPOCH}; 

    const RTP_VERSION: u8 = 2;
    const MIDI_PAYLOAD_TYPE: u8 = 97;

    #[derive(Debug, Clone, Serialize, Deserialize)]
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
        
        midi_commands: Vec<MidiMessage>,
    }

    impl RtpMidiPacket {
        pub fn create(midi_messages: Vec<MidiMessage>) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis() as u32;
            
            Self {
                version: RTP_VERSION,
                padding: false,
                extension: false,
                csrc_count: 0,
                marker: false,
                payload_type: MIDI_PAYLOAD_TYPE,
                sequence_number: 0, 
                timestamp,
                ssrc: 0,
                journal_present: false,
                midi_commands: midi_messages,
            }
        }
        
        pub fn parse(data: &[u8]) -> Result<Self> {
            if data.len() < 12 {
                return Err(anyhow::anyhow!("Data too short for RTP header ({} bytes)", data.len()));
            }
            
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
            
            let header_size = 12 + (csrc_count as usize) * 4;
            if extension {
                 return Err(anyhow::anyhow!("RTP extension headers not supported"));
            }
            
            if data.len() <= header_size {
                 return Err(anyhow::anyhow!("Data too short for MIDI payload"));
            }
            
            let midi_payload = &data[header_size..];
            let midi_header = midi_payload[0];
            let journal_present = (midi_header >> 7) & 1 != 0;
            let _b_bit = (midi_header >> 6) & 1 != 0;
            let _midi_list_len = (midi_header & 0x0F) as usize;

            let mut midi_commands = Vec::new();
            let mut offset = 1;
            
            let mut running_status: Option<u8> = None;

            while offset < midi_payload.len() {
                let (delta_time, bytes_read) = parse_variable_length_quantity(&midi_payload[offset..])?;
                offset += bytes_read;

                if offset >= midi_payload.len() {
                    break;
                }
                
                let first_byte = midi_payload[offset];
                let command_byte;
                let mut command_data = Vec::new();

                if first_byte >= 0x80 {
                    command_byte = first_byte;
                    command_data.push(command_byte);
                    offset += 1;
                    if command_byte < 0xF0 {
                       running_status = Some(command_byte);
                    } else {
                       running_status = None;
                    }
                } else {
                    command_byte = running_status.ok_or_else(|| anyhow::anyhow!("Missing running status"))?;
                    command_data.push(command_byte);
                }

                let data_len = midi_command_length(command_byte);
                
                if offset + data_len > midi_payload.len() {
                    return Err(anyhow::anyhow!("Incomplete MIDI command data"));
                }
                
                for _ in 0..data_len {
                    command_data.push(midi_payload[offset]);
                    offset += 1;
                }
                
                midi_commands.push(MidiMessage::new(delta_time, command_data));
            }
            
            Ok(Self {
                version, padding, extension, csrc_count, marker, payload_type,
                sequence_number, timestamp, ssrc,
                journal_present,
                midi_commands,
            })
        }
        
        pub fn serialize(&self) -> Result<Bytes> {
            let mut buffer = BytesMut::with_capacity(1024);
            
            let b0 = (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4) | (self.csrc_count & 0x0F);
            let b1 = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
            buffer.extend_from_slice(&[b0, b1]);
            buffer.extend_from_slice(&self.sequence_number.to_be_bytes());
            buffer.extend_from_slice(&self.timestamp.to_be_bytes());
            buffer.extend_from_slice(&self.ssrc.to_be_bytes());
            
            let midi_header = ((self.journal_present as u8) << 7) | (1 << 6); 
            buffer.extend_from_slice(&[midi_header]);

            let mut midi_payload = BytesMut::new();
            for msg in &self.midi_commands {
                let mut delta_time_buf = [0u8; 4];
                let dt_len = encode_variable_length_quantity(msg.delta_time, &mut delta_time_buf)?;
                midi_payload.extend_from_slice(&delta_time_buf[..dt_len]);
                midi_payload.extend_from_slice(&msg.command);
            }
            
            let len_byte_index = 12;
            let midi_list_len = self.midi_commands.len();
            if midi_list_len > 15 {
                return Err(anyhow::anyhow!("Too many MIDI commands in one packet (max 15)"));
            }
            buffer[len_byte_index] |= midi_list_len as u8;

            buffer.extend_from_slice(&midi_payload);
            
            Ok(buffer.freeze())
        }
        
        pub fn midi_commands(&self) -> &Vec<MidiMessage> { &self.midi_commands }
        pub fn set_sequence_number(&mut self, seq: u16) { self.sequence_number = seq; }
        pub fn set_ssrc(&mut self, ssrc: u32) { self.ssrc = ssrc; }
        pub fn set_journal_present(&mut self, present: bool) { self.journal_present = present; }
        pub fn sequence_number(&self) -> u16 { self.sequence_number }
        pub fn ssrc(&self) -> u32 { self.ssrc }
        pub fn journal_present(&self) -> bool { self.journal_present }
    }

    fn parse_variable_length_quantity(data: &[u8]) -> Result<(u32, usize)> {
        let mut value: u32 = 0;
        let mut bytes_read = 0;
        for (i, &byte) in data.iter().enumerate() {
            if i >= 4 { return Err(anyhow::anyhow!("VLQ too long")); }
            value = (value << 7) | (byte & 0x7F) as u32;
            bytes_read += 1;
            if byte & 0x80 == 0 { return Ok((value, bytes_read)); }
        }
        Err(anyhow::anyhow!("Incomplete VLQ data"))
    }

    fn encode_variable_length_quantity(value: u32, buf: &mut [u8; 4]) -> Result<usize> {
        if value > 0x0FFFFFFF { return Err(anyhow::anyhow!("Value too large for VLQ encoding")); }
        let mut temp = value;
        let mut bytes = [0u8; 4];
        let mut i = 3;
        loop {
            bytes[i] = (temp & 0x7F) as u8;
            temp >>= 7;
            if i < 3 { bytes[i] |= 0x80; }
            if temp == 0 { break; }
            if i == 0 { return Err(anyhow::anyhow!("VLQ encoding error")); }
            i -= 1;
        }
        let len = 4 - i;
        buf[..len].copy_from_slice(&bytes[i..]);
        Ok(len)
    }
    
    fn midi_command_length(status_byte: u8) -> usize {
        match status_byte & 0xF0 {
            0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => 2,
            0xC0 | 0xD0 => 1,
            0xF0 => match status_byte { 0xF1 | 0xF3 => 1, 0xF2 => 2, _ => 0, },
            _ => 0,
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
        pub fn new() -> Self { Self { peers: Arc::new(Mutex::new(HashMap::new())), } }
        pub fn register(&self, client_id: String, peer_type: super::PeerType, tx: mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>) { self.peers.lock().unwrap().insert(client_id, (peer_type, tx)); }
        pub fn unregister(&self, client_id: &str) { self.peers.lock().unwrap().remove(client_id); }
        pub fn get_audio_server(&self) -> Option<(String, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)> {
            let peers = self.peers.lock().unwrap();
            for (id, (peer_type, tx)) in peers.iter() {
                if let super::PeerType::AudioServer = peer_type { return Some((id.clone(), tx.clone())); }
            }
            None
        }
        pub fn get_peer(&self, client_id: &str) -> Option<mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>> { self.peers.lock().unwrap().get(client_id).map(|(_, tx)| tx.clone()) }
        pub fn get_all_clients(&self) -> Vec<(String, super::PeerType)> { self.peers.lock().unwrap().iter().map(|(id, (peer_type, _))| (id.clone(), peer_type.clone())).collect() }
    }

    pub async fn handle_connection(clients: Clients, stream: TcpStream) {
        info!("New connection from {}", stream.peer_addr().unwrap());
        let ws_stream = match accept_async(stream).await { Ok(ws) => ws, Err(e) => { error!("Error during WebSocket handshake: {}", e); return; } };
        let (mut ws_write, mut ws_read) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<Result<Message, tokio_tungstenite::tungstenite::Error>>(64);
        let mut client_id = String::new();
        let mut _peer_type = None;
        tokio::spawn(async move { while let Some(msg) = rx.recv().await { if let Err(e) = ws_write.send(msg.unwrap()).await { error!("Error sending message over WebSocket: {}", e); break; } } });
        while let Some(msg) = ws_read.next().await {
            let msg = match msg { Ok(msg) => msg, Err(e) => { error!("Error reading message from WebSocket: {}", e); break; } };
            if msg.is_close() { info!("WebSocket connection closed by peer"); break; }
            if let Message::Text(text) = msg {
                match serde_json::from_str::<SignalingMessage>(&text) {
                    Ok(signaling_msg) => match signaling_msg.message_type.as_str() {
                        "register" => {
                            if let Ok(register_payload) = serde_json::from_value::<RegisterPayload>(signaling_msg.payload) {
                                client_id = register_payload.client_id.clone();
                                _peer_type = Some(register_payload.peer_type.clone());
                                clients.register(client_id.clone(), register_payload.peer_type, tx.clone());
                                info!("Registered client: {} (type: {:?})", client_id, _peer_type);
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
                                if let Some(super::PeerType::ClientApp) = _peer_type {
                                    notify_audio_server_of_new_client(&clients, &client_id).await;
                                }
                            } else {
                                warn!("Failed to parse register payload from client {}", client_id);
                            }
                        },
                        "list_peers" => {
                            let response = SignalingMessage {
                                message_type: "peer_list".to_string(),
                                sender_id: "server".to_string(),
                                receiver_id: Some(client_id.clone()),
                                payload: json!({
                                    "peers": clients.get_all_clients()
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
                    },
                    Err(e) => {
                        error!("Failed to parse signaling message: {}", e);
                    }
                }
            } else if msg.is_binary() {
                warn!("Received binary message, expected text");
            }
        }
        if !client_id.is_empty() { info!("Client {} disconnected", client_id); clients.unregister(&client_id); }
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
            let clients_clone = clients.clone();
            tokio::spawn(async move { handle_connection(clients_clone, stream).await; });
        }
        Ok(())
    }
}

pub mod midi {
    pub mod rtp {
        pub use super::super::midi_rtp_packet::*;
    }
    pub mod message {
        pub use super::super::MidiMessage;
    }
}
pub mod signaling_server {
    pub use super::signaling_server_module::*;
}

pub mod wled_control;

/// Main logic loop for audio processing and DDP output.
/// This function is shared between the desktop main.rs and the Android AIDL service.
pub fn run_service_loop(config: Config, running: Arc<AtomicBool>) {
    info!("Core service loop started.");
    let mut handles = vec![];

    // --- Channels for inter-thread communication ---
    let (audio_tx, audio_rx) = crossbeam_channel::bounded(8);
    let (fft_tx, fft_rx) = crossbeam_channel::bounded(8);
    let (led_tx, led_rx) = crossbeam_channel::bounded(8);
    let (midi_tx, midi_rx) = crossbeam_channel::bounded::<MidiMessage>(32);

    // --- Audio-to-DDP Pipeline ---
    let audio_config = config.clone();
    let running_audio = running.clone();
    let audio_handle = thread::spawn(move || {
        match start_audio_input(audio_config.audio_device.as_deref(), audio_tx) {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    error!("Failed to play audio stream: {}", e);
                    return;
                }
                info!("Audio input stream started.");
                while running_audio.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(100));
                }
                info!("Audio input stream stopping.");
            }
            Err(e) => error!("Failed to start audio input: {}", e),
        }
    });
    handles.push(audio_handle);

    let running_analysis = running.clone();
    let analysis_handle = thread::spawn(move || {
        let mut prev = Vec::new();
        let smoothing = 0.7;
        while running_analysis.load(Ordering::Relaxed) {
             if let Ok(buffer) = audio_rx.recv_timeout(Duration::from_secs(1)) {
                let mags = compute_fft_magnitudes(&buffer, &mut prev, smoothing);
                if fft_tx.send(mags).is_err() { break; }
            }
        }
        info!("Audio analysis thread stopped.");
    });
    handles.push(analysis_handle);

    let led_count_mapping = config.led_count;
    let running_mapping = running.clone();
    let mapping_handle = thread::spawn(move || {
        while running_mapping.load(Ordering::Relaxed) {
            if let Ok(mags) = fft_rx.recv_timeout(Duration::from_secs(1)) {
                let leds = map_audio_to_leds(&mags, led_count_mapping);
                if led_tx.send(leds).is_err() { break; }
            }
        }
        info!("Light mapping thread stopped.");
    });
    handles.push(mapping_handle);

    let ddp_config = config.clone();
    let running_ddp = running.clone();
    let ddp_handle = thread::spawn(move || {
        let rgbw = ddp_config.color_format.as_deref().unwrap_or("RGB").eq_ignore_ascii_case("RGBW");
        let mut sender = match create_ddp_sender(&ddp_config.wled_ip, ddp_config.ddp_port.unwrap_or(4048), ddp_config.led_count, rgbw) {
            Ok(s) => s,
            Err(e) => { error!("Failed to create DDP sender: {}", e); return; }
        };
        info!("DDP sender created for {}", ddp_config.wled_ip);
        while running_ddp.load(Ordering::Relaxed) {
            if let Ok(leds) = led_rx.recv_timeout(Duration::from_millis(100)) {
                if let Err(e) = send_ddp_frame(&mut sender, &leds) {
                    warn!("Failed to send DDP frame: {}", e);
                }
            }
        }
        info!("DDP output thread stopped.");
    });
    handles.push(ddp_handle);


    // --- MIDI-to-WLED Pipeline ---
    let midi_config = config.clone();
    let running_midi_listener = running.clone();
    let midi_listener_handle = thread::spawn(move || {
        let rt = match Runtime::new() {
            Ok(rt) => rt,
            Err(e) => { error!("Failed to create Tokio runtime for MIDI listener: {}", e); return; }
        };

        rt.block_on(async move {
            let port = midi_config.midi_port.unwrap_or(5004);
            let addr = format!("0.0.0.0:{}", port);
            let sock = match UdpSocket::bind(&addr).await {
                Ok(s) => s,
                Err(e) => { error!("Failed to bind MIDI UDP socket on {}: {}", addr, e); return; }
            };
            info!("MIDI listener started on UDP port {}", port);
            let mut buf = [0; 1024];

            while running_midi_listener.load(Ordering::Relaxed) {
                if let Ok(Ok((len, _))) = tokio::time::timeout(Duration::from_secs(1), sock.recv_from(&mut buf)).await {
                    match midi::rtp::RtpMidiPacket::parse(&buf[..len]) {
                        Ok(packet) => {
                            for command in packet.midi_commands() {
                                if midi_tx.send(command.clone()).is_err() {
                                    warn!("MIDI channel closed, stopping listener.");
                                    return;
                                }
                            }
                        }
                        Err(e) => warn!("Failed to parse RTP-MIDI packet: {}", e),
                    }
                }
            }
            info!("MIDI listener thread stopped.");
        });
    });
    handles.push(midi_listener_handle);

    let wled_ip_midi = config.wled_ip.clone();
    let running_midi_logic = running.clone();
    let midi_logic_handle = thread::spawn(move || {
         let rt = match Runtime::new() {
            Ok(rt) => rt,
            Err(e) => { error!("Failed to create Tokio runtime for MIDI logic: {}", e); return; }
        };

        while running_midi_logic.load(Ordering::Relaxed) {
            if let Ok(midi_msg) = midi_rx.recv_timeout(Duration::from_secs(1)) {
                let command_type = midi_msg.command.get(0).cloned().unwrap_or(0) & 0xF0;
                match command_type {
                    0x90 => { // Note On
                        let note = midi_msg.command.get(1).cloned().unwrap_or(0);
                        let velocity = midi_msg.command.get(2).cloned().unwrap_or(0);
                        if velocity > 0 {
                            info!("Note On: {}, Vel: {}", note, velocity);
                            // Map note to a color (simple hue rotation)
                            let hue = (note % 12) * 30;
                            let (r,g,b) = hsv_to_rgb(hue as f32, 1.0, 1.0);
                            let ip = wled_ip_midi.clone();
                            rt.block_on(async move {
                                if let Err(e) = wled_control::set_wled_color(&ip, r, g, b).await {
                                    warn!("Failed to set WLED color via MIDI: {}", e);
                                }
                            });
                        } else {
                            // Note On with velocity 0 is often treated as Note Off
                             info!("Note Off (via Vel 0): {}", note);
                        }
                    },
                    0xB0 => { // Control Change
                        let cc_num = midi_msg.command.get(1).cloned().unwrap_or(0);
                        let cc_val = midi_msg.command.get(2).cloned().unwrap_or(0);
                        info!("CC: #{}, Val: {}", cc_num, cc_val);
                        if cc_num == 1 { // Typically modulation wheel
                             // Map CC value to a preset ID (1-255)
                             let preset_id = (cc_val as f32 / 127.0 * 254.0).round() as i32 + 1;
                             let ip = wled_ip_midi.clone();
                              rt.block_on(async move {
                                if let Err(e) = wled_control::set_wled_preset(&ip, preset_id).await {
                                    warn!("Failed to set WLED preset via MIDI: {}", e);
                                }
                            });
                        }
                    },
                    _ => {}
                }
            }
        }
        info!("MIDI logic thread stopped.");
    });
    handles.push(midi_logic_handle);

    // --- Wait for all threads to finish ---
    for handle in handles {
        handle.join().expect("A service thread panicked");
    }
    info!("All service threads have been joined.");
}

// Helper to convert HSV to RGB for colorful MIDI notes
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r_prime, g_prime, b_prime) = if h >= 0.0 && h < 60.0 {
        (c, x, 0.0)
    } else if h >= 60.0 && h < 120.0 {
        (x, c, 0.0)
    } else if h >= 120.0 && h < 180.0 {
        (0.0, c, x)
    } else if h >= 180.0 && h < 240.0 {
        (0.0, x, c)
    } else if h >= 240.0 && h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r_prime + m) * 255.0) as u8,
        ((g_prime + m) * 255.0) as u8,
        ((b_prime + m) * 255.0) as u8,
    )
}

pub mod ffi;
