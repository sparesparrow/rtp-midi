use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use bytes::{Buf, Bytes};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[derive(Debug, Clone)]
pub enum Event {
    AudioDataReady(Vec<f32>),
    MidiMessageReceived(Vec<u8>),
    RawPacketReceived { source: String, data: Vec<u8> },
    SendPacket { destination: String, port: u16, data: Vec<u8> },
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
    // Midi(MidiCommand), // This will need to be handled for cross-crate
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WledOutputAction {
    SetPreset { id: i32 },
    SetBrightness { value: u8 },
    SetColor { r: u8, g: u8, b: u8 },
    SetEffect { id: i32, speed: Option<u8>, intensity: Option<u8> },
    SetPalette { id: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MappingOutput {
    Wled(WledOutputAction),
    // Ddp(DdpOutputAction), // Připravte pro další typy výstupů
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    pub input: InputEvent,
    pub output: Vec<MappingOutput>,
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
    let status_byte = reader.chunk()[0]; // Peek to handle running status later if needed

    // Check for running status (data byte without preceding status byte)
    if status_byte < 0x80 {
        return Err(anyhow!("Running status not supported in this parser (yet)."));
    }

    let command_length = midi_command_length(status_byte)?;
    if reader.len() < command_length {
        return Err(anyhow!("Incomplete MIDI message: Expected {} bytes, got {}", command_length, reader.len()));
    }

    let command_slice = reader.copy_to_bytes(command_length);
    let mut command_reader = command_slice;
    let command = MidiCommand::parse(&mut command_reader)?;
    Ok((command, command_length))
}

pub fn midi_command_length(status_byte: u8) -> Result<usize> {
    match status_byte & 0xF0 {
        0x80 => Ok(3), // Note Off
        0x90 => Ok(3), // Note On
        0xA0 => Ok(3), // Polyphonic Key Pressure
        0xB0 => Ok(3), // Control Change
        0xC0 => Ok(2), // Program Change
        0xD0 => Ok(2), // Channel Pressure
        0xE0 => Ok(3), // Pitch Bend Change
        0xF0 => {
            match status_byte {
                0xF0 => Ok(1), // SysEx Start (length is variable, handled by subsequent bytes)
                0xF1 => Ok(2), // MTC Quarter Frame
                0xF2 => Ok(3), // Song Position Pointer
                0xF3 => Ok(2), // Song Select
                0xF6 => Ok(1), // Tune Request
                0xF8 => Ok(1), // Timing Clock
                0xFA => Ok(1), // Start
                0xFB => Ok(1), // Continue
                0xFC => Ok(1), // Stop
                0xFE => Ok(1), // Active Sensing
                0xFF => Ok(1), // Reset
                _ => Err(anyhow!("Unknown System Common/Real-Time message: 0x{:X}", status_byte)),
            }
        }
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
                Ok(MidiCommand::PitchBendChange {
                    channel: status_byte & 0x0F,
                    value: ((msb as u16) << 7) | (lsb as u16),
                })
            }
            0xF0 => {
                match status_byte {
                    0xF0 => {
                        // SysEx Start - read until 0xF7 (End of SysEx)
                        let mut sysex_data = Vec::new();
                        while data.has_remaining() && data[0] != 0xF7 {
                            sysex_data.push(data.get_u8());
                        }
                        if data.is_empty() { // Reached end of data without 0xF7
                            return Err(anyhow!("Incomplete SysEx message: Missing 0xF7"));
                        }
                        data.get_u8(); // Consume 0xF7
                        Ok(MidiCommand::SystemExclusive(sysex_data))
                    }
                    0xF1 => {
                        // MTC Quarter Frame (1 data byte)
                        if data.is_empty() { return Err(anyhow!("Incomplete MTC Quarter Frame")); }
                        Ok(MidiCommand::Unknown { status: status_byte, data: vec![data.get_u8()] })
                    }
                    0xF2 => {
                        // Song Position Pointer (2 data bytes)
                        if data.len() < 2 { return Err(anyhow!("Incomplete Song Position Pointer")); }
                        Ok(MidiCommand::Unknown { status: status_byte, data: vec![data.get_u8(), data.get_u8()] })
                    }
                    0xF3 => {
                        // Song Select (1 data byte)
                        if data.is_empty() { return Err(anyhow!("Incomplete Song Select")); }
                        Ok(MidiCommand::Unknown { status: status_byte, data: vec![data.get_u8()] })
                    }
                    0xF6 => Ok(MidiCommand::TuneRequest),
                    0xF8 => Ok(MidiCommand::TimingClock),
                    0xFA => Ok(MidiCommand::Start),
                    0xFB => Ok(MidiCommand::Continue),
                    0xFC => Ok(MidiCommand::Stop),
                    0xFE => Ok(MidiCommand::ActiveSensing),
                    0xFF => Ok(MidiCommand::Unknown { status: status_byte, data: vec![] }), // Reset
                    _ => Err(anyhow!("Unknown System Common/Real-Time message: 0x{:X}", status_byte)),
                }
            }
            _ => Err(anyhow!("Unknown MIDI status byte: 0x{:X}", status_byte)),
        }
    }
}

pub fn parse_rtp_packet(data: &[u8]) -> Result<ParsedPacket> {
    let mut reader = bytes::Bytes::copy_from_slice(data);
    if reader.len() < 12 {
        return Err(anyhow::anyhow!("RTP header too short"));
    }

    let byte0 = reader.get_u8();
    let version = (byte0 >> 6) & 0x03;
    if version != 2 {
        return Err(anyhow::anyhow!("Unsupported RTP version: {}", version));
    }
    let padding = (byte0 >> 5) & 0x01 == 1;
    let extension = (byte0 >> 4) & 0x01 == 1;
    
    let byte1 = reader.get_u8();
    let marker = (byte1 >> 7) & 0x01 == 1;
    let payload_type = byte1 & 0x7F;

    let sequence_number = reader.get_u16();
    let timestamp = reader.get_u32();
    let ssrc = reader.get_u32();
    
    // Skip CSRC
    let csrc_count = byte0 & 0x0F;
    reader.advance(csrc_count as usize * 4);

    let payload = reader.copy_to_bytes(reader.remaining()).to_vec();

    Ok(ParsedPacket {
        version, padding, extension, marker, payload_type,
        sequence_number, timestamp, ssrc,
        payload,
    })
}

impl Mapping {
    pub fn matches_midi_command(&self, command: &MidiCommand) -> bool {
        match (&self.input, command) {
            (InputEvent::MidiNoteOn { note, velocity: note_vel }, MidiCommand::NoteOn { channel: _, key, velocity: cmd_vel }) => {
                (note.is_none() || *note == Some(*key))
                    && (note_vel.is_none() || *note_vel == Some(*cmd_vel))
            },
            (InputEvent::MidiControlChange { controller, value }, MidiCommand::ControlChange { channel: _, control, value: cc_val }) => {
                (controller.is_none() || *controller == Some(*control))
                    && (value.is_none() || *value == Some(*cc_val))
            },
            _ => false,
        }
    }
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
