#![deny(warnings)]

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod event_bus;
pub mod journal_engine;
pub mod mapping;
pub mod network_interface;
pub mod packet_processor;
pub mod session_manager;

use std::fmt;

/// Chyba při práci se streamem.
///
/// Používá se ve všech implementacích DataStreamNetSender/Receiver pro sjednocené API napříč platformami.
#[derive(Debug)]
pub enum StreamError {
    /// IO chyba (např. socket, soubor)
    Io(std::io::Error),
    /// Síťová chyba (např. unreachable, timeout)
    Network(String),
    /// Ostatní chyby
    Other(String),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::Io(e) => write!(f, "IO chyba: {e}"),
            StreamError::Network(s) => write!(f, "Síťová chyba: {s}"),
            StreamError::Other(s) => write!(f, "Jiná chyba: {s}"),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<std::io::Error> for StreamError {
    fn from(e: std::io::Error) -> Self {
        StreamError::Io(e)
    }
}

/// Trait pro odesílače datových streamů (síť, HW, ...).
///
/// Implementujte pro každý typ výstupu (WLED, DDP, DMX, ...).
/// Umožňuje jednotné API napříč platformami a buildy.
pub trait DataStreamNetSender {
    /// Inicializace zařízení/zdroje (volitelné, lze nechat prázdné)
    fn init(&mut self) -> Result<(), StreamError>;
    /// Odeslání datového paketu s timestampem
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError>;
    /// Defaultní metoda pro sdílenou logiku (např. fragmentace, retry)
    fn send_raw(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.send(ts, payload)
    }
}

/// Trait pro přijímače datových streamů (síť, HW, ...).
///
/// Implementujte pro každý typ vstupu (RTP-MIDI, DDP, ...).
pub trait DataStreamNetReceiver {
    /// Inicializace přijímače
    fn init(&mut self) -> Result<(), StreamError>;
    /// Čtení/polling dat do bufferu, vrací timestamp a délku
    fn poll(&mut self, buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError>;
}

/// Mock implementace DataStreamNetSender pro testování a dependency injection.
///
/// Umožňuje testovat logiku bez skutečného síťového/hardware výstupu.
/// Příklad použití v testu viz níže.
pub struct MockSender {
    pub sent: Vec<(u64, Vec<u8>)>,
}

impl Default for MockSender {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSender {
    pub fn new() -> Self {
        Self { sent: Vec::new() }
    }
}

impl DataStreamNetSender for MockSender {
    fn init(&mut self) -> Result<(), StreamError> {
        Ok(())
    }
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.sent.push((ts, payload.to_vec()));
        Ok(())
    }
}

/*
Příklad použití v testu:

#[test]
fn test_sender_injection() {
    let mut sender = MockSender::new();
    sender.send(123, b"test").unwrap();
    assert_eq!(sender.sent.len(), 1);
    assert_eq!(sender.sent[0].1, b"test");
}
*/

pub use crate::journal_engine::{JournalData, JournalEntry};

// === Shared Data Models and MIDI Parsing Logic (moved from utils) ===

use anyhow::{anyhow, Result};
use bytes::{Buf, Bytes};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Event {
    AudioDataReady(Vec<f32>),
    MidiMessageReceived(Vec<u8>),
    RawPacketReceived {
        source: String,
        data: Vec<u8>,
    },
    SendPacket {
        destination: String,
        port: u16,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputEvent {
    MidiNoteOn {
        note: Option<u8>,
        velocity: Option<u8>,
    },
    MidiControlChange {
        controller: Option<u8>,
        value: Option<u8>,
    },
    AudioPeak,
    AudioBand {
        band: String,
        threshold: Option<f32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WledOutputAction {
    SetPreset {
        id: i32,
    },
    SetBrightness {
        value: u8,
    },
    SetColor {
        r: u8,
        g: u8,
        b: u8,
    },
    SetEffect {
        id: i32,
        speed: Option<u8>,
        intensity: Option<u8>,
    },
    SetPalette {
        id: i32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MappingOutput {
    Wled(WledOutputAction),
    // Ddp(DdpOutputAction), // Extend for other output types
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    pub input: InputEvent,
    pub output: Vec<MappingOutput>,
}

impl Mapping {
    /// Returns true if this mapping matches the given MIDI command.
    pub fn matches_midi_command(&self, command: &MidiCommand) -> bool {
        match (&self.input, command) {
            (
                InputEvent::MidiNoteOn {
                    note,
                    velocity: note_vel,
                },
                MidiCommand::NoteOn {
                    channel: _,
                    key,
                    velocity: cmd_vel,
                },
            ) => {
                (note.is_none() || *note == Some(*key))
                    && (note_vel.is_none() || *note_vel == Some(*cmd_vel))
            }
            (
                InputEvent::MidiControlChange { controller, value },
                MidiCommand::ControlChange {
                    channel: _,
                    control,
                    value: cc_val,
                },
            ) => {
                (controller.is_none() || *controller == Some(*control))
                    && (value.is_none() || *value == Some(*cc_val))
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedPacket {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Vec<u8>,
}

/// Parses a raw RTP packet into a ParsedPacket struct.
pub fn parse_rtp_packet(data: &[u8]) -> Result<ParsedPacket> {
    if data.len() < 12 {
        return Err(anyhow!("RTP packet too short: {} bytes", data.len()));
    }
    let mut reader = Bytes::copy_from_slice(data);
    let b0 = reader.get_u8();
    let version = (b0 >> 6) & 0x03;
    let padding = (b0 & 0x20) != 0;
    let extension = (b0 & 0x10) != 0;
    // let csrc_count = b0 & 0x0F; // Not used
    let b1 = reader.get_u8();
    let marker = (b1 & 0x80) != 0;
    let payload_type = b1 & 0x7F;
    let sequence_number = reader.get_u16();
    let timestamp = reader.get_u32();
    let ssrc = reader.get_u32();
    // Skip CSRCs and header extensions for now (not used in RTP-MIDI)
    let payload = reader.copy_to_bytes(reader.remaining()).to_vec();
    Ok(ParsedPacket {
        version,
        padding,
        extension,
        marker,
        payload_type,
        sequence_number,
        timestamp,
        ssrc,
        payload,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MidiCommand {
    NoteOff { channel: u8, key: u8, velocity: u8 },
    NoteOn { channel: u8, key: u8, velocity: u8 },
    PolyphonicKeyPressure { channel: u8, key: u8, value: u8 },
    ControlChange { channel: u8, control: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    ChannelPressure { channel: u8, value: u8 },
    PitchBendChange { channel: u8, value: u16 },
    TimingClock,
    Start,
    Continue,
    Stop,
    ActiveSensing,
    TuneRequest,
    SystemExclusive(Vec<u8>),
    Unknown { status: u8, data: Vec<u8> },
}

pub fn parse_midi_message(data: &[u8]) -> Result<(MidiCommand, usize)> {
    if data.is_empty() {
        return Err(anyhow!("Empty MIDI data"));
    }
    let mut reader = Bytes::copy_from_slice(data);
    let status_byte = reader.chunk()[0];
    if status_byte < 0x80 {
        return Err(anyhow!(
            "Running status not supported in this parser (yet)."
        ));
    }
    let command_length = midi_command_length(status_byte)?;
    if reader.len() < command_length {
        return Err(anyhow!(
            "Incomplete MIDI message: Expected {} bytes, got {}",
            command_length,
            reader.len()
        ));
    }
    let command_slice = reader.copy_to_bytes(command_length);
    let mut command_reader = command_slice;
    let command = MidiCommand::parse(&mut command_reader)?;
    Ok((command, command_length))
}

pub fn midi_command_length(status_byte: u8) -> Result<usize> {
    match status_byte & 0xF0 {
        0x80 => Ok(3),
        0x90 => Ok(3),
        0xA0 => Ok(3),
        0xB0 => Ok(3),
        0xC0 => Ok(2),
        0xD0 => Ok(2),
        0xE0 => Ok(3),
        0xF0 => match status_byte {
            0xF0 => Ok(1),
            0xF1 => Ok(2),
            0xF2 => Ok(3),
            0xF3 => Ok(2),
            0xF6 => Ok(1),
            0xF8 => Ok(1),
            0xFA => Ok(1),
            0xFB => Ok(1),
            0xFC => Ok(1),
            0xFE => Ok(1),
            0xFF => Ok(1),
            _ => Err(anyhow!(
                "Unknown System Common/Real-Time message: 0x{:X}",
                status_byte
            )),
        },
        _ => Err(anyhow!("Unknown MIDI status byte: 0x{:X}", status_byte)),
    }
}

impl MidiCommand {
    pub fn parse(data: &mut Bytes) -> Result<Self> {
        if data.is_empty() {
            return Err(anyhow!("Empty MIDI data"));
        }
        let status_byte = data.get_u8();
        match status_byte & 0xF0 {
            0x80 => {
                if data.len() < 2 {
                    return Err(anyhow!("Incomplete Note Off message"));
                }
                let key = data.get_u8();
                let velocity = data.get_u8();
                Ok(MidiCommand::NoteOff {
                    channel: status_byte & 0x0F,
                    key,
                    velocity,
                })
            }
            0x90 => {
                if data.len() < 2 {
                    return Err(anyhow!("Incomplete Note On message"));
                }
                let key = data.get_u8();
                let velocity = data.get_u8();
                Ok(MidiCommand::NoteOn {
                    channel: status_byte & 0x0F,
                    key,
                    velocity,
                })
            }
            0xA0 => {
                if data.len() < 2 {
                    return Err(anyhow!("Incomplete Polyphonic Key Pressure message"));
                }
                let key = data.get_u8();
                let value = data.get_u8();
                Ok(MidiCommand::PolyphonicKeyPressure {
                    channel: status_byte & 0x0F,
                    key,
                    value,
                })
            }
            0xB0 => {
                if data.len() < 2 {
                    return Err(anyhow!("Incomplete Control Change message"));
                }
                let control = data.get_u8();
                let value = data.get_u8();
                Ok(MidiCommand::ControlChange {
                    channel: status_byte & 0x0F,
                    control,
                    value,
                })
            }
            0xC0 => {
                if data.is_empty() {
                    return Err(anyhow!("Incomplete Program Change message"));
                }
                let program = data.get_u8();
                Ok(MidiCommand::ProgramChange {
                    channel: status_byte & 0x0F,
                    program,
                })
            }
            0xD0 => {
                if data.is_empty() {
                    return Err(anyhow!("Incomplete Channel Pressure message"));
                }
                let value = data.get_u8();
                Ok(MidiCommand::ChannelPressure {
                    channel: status_byte & 0x0F,
                    value,
                })
            }
            0xE0 => {
                if data.len() < 2 {
                    return Err(anyhow!("Incomplete Pitch Bend Change message"));
                }
                let lsb = data.get_u8();
                let msb = data.get_u8();
                let value = ((msb as u16) << 7) | (lsb as u16);
                Ok(MidiCommand::PitchBendChange {
                    channel: status_byte & 0x0F,
                    value,
                })
            }
            0xF0 => {
                match status_byte {
                    0xF0 => {
                        // SysEx Start (variable length, not fully handled here)
                        let mut sysex = Vec::new();
                        while let Some(&_b) = data.chunk().first() {
                            let b = data.get_u8();
                            sysex.push(b);
                            if b == 0xF7 {
                                break;
                            }
                        }
                        Ok(MidiCommand::SystemExclusive(sysex))
                    }
                    0xF6 => Ok(MidiCommand::TuneRequest),
                    0xF8 => Ok(MidiCommand::TimingClock),
                    0xFA => Ok(MidiCommand::Start),
                    0xFB => Ok(MidiCommand::Continue),
                    0xFC => Ok(MidiCommand::Stop),
                    0xFE => Ok(MidiCommand::ActiveSensing),
                    _ => Ok(MidiCommand::Unknown {
                        status: status_byte,
                        data: data.to_vec(),
                    }),
                }
            }
            _ => Ok(MidiCommand::Unknown {
                status: status_byte,
                data: data.to_vec(),
            }),
        }
    }
}

// --- Config struct (moved from rtp_midi_lib) ---
#[derive(Debug, Clone, Deserialize)]
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
    pub mapping_preset: Option<String>,
    // Android Hub specific fields
    pub esp32_ip: Option<String>,
    pub esp32_port: Option<u16>,
    pub daw_ip: Option<String>,
    pub daw_port: Option<u16>,
}

impl Config {
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
