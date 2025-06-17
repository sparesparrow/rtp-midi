use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::warn;
use serde::{Deserialize, Serialize};

/// Represents a MIDI command.
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
    // Add more MIDI commands as needed
    Unknown { status: u8, data: Vec<u8> },
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
                if data.len() < 1 {
                    return Err(anyhow!("Incomplete Program Change message"));
                }
                let program = data.get_u8();
                Ok(MidiCommand::ProgramChange {
                    channel: status_byte & 0x0F,
                    program,
                })
            }
            0xD0 => {
                if data.len() < 1 {
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
                        if data.len() < 1 { return Err(anyhow!("Incomplete MTC Quarter Frame")); }
                        Ok(MidiCommand::Unknown { status: status_byte, data: vec![data.get_u8()] })
                    }
                    0xF2 => {
                        // Song Position Pointer (2 data bytes)
                        if data.len() < 2 { return Err(anyhow!("Incomplete Song Position Pointer")); }
                        Ok(MidiCommand::Unknown { status: status_byte, data: vec![data.get_u8(), data.get_u8()] })
                    }
                    0xF3 => {
                        // Song Select (1 data byte)
                        if data.len() < 1 { return Err(anyhow!("Incomplete Song Select")); }
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

    pub fn write_to_bytes(&self, buf: &mut BytesMut) {
        match self {
            MidiCommand::NoteOff { channel, key, velocity } => {
                buf.put_u8(0x80 | (channel & 0x0F));
                buf.put_u8(*key);
                buf.put_u8(*velocity);
            }
            MidiCommand::NoteOn { channel, key, velocity } => {
                buf.put_u8(0x90 | (channel & 0x0F));
                buf.put_u8(*key);
                buf.put_u8(*velocity);
            }
            MidiCommand::PolyphonicKeyPressure { channel, key, value } => {
                buf.put_u8(0xA0 | (channel & 0x0F));
                buf.put_u8(*key);
                buf.put_u8(*value);
            }
            MidiCommand::ControlChange { channel, control, value } => {
                buf.put_u8(0xB0 | (channel & 0x0F));
                buf.put_u8(*control);
                buf.put_u8(*value);
            }
            MidiCommand::ProgramChange { channel, program } => {
                buf.put_u8(0xC0 | (channel & 0x0F));
                buf.put_u8(*program);
            }
            MidiCommand::ChannelPressure { channel, value } => {
                buf.put_u8(0xD0 | (channel & 0x0F));
                buf.put_u8(*value);
            }
            MidiCommand::PitchBendChange { channel, value } => {
                buf.put_u8(0xE0 | (channel & 0x0F));
                buf.put_u8((*value & 0x7F) as u8);
                buf.put_u8(((*value >> 7) & 0x7F) as u8);
            }
            MidiCommand::TimingClock => buf.put_u8(0xF8),
            MidiCommand::Start => buf.put_u8(0xFA),
            MidiCommand::Continue => buf.put_u8(0xFB),
            MidiCommand::Stop => buf.put_u8(0xFC),
            MidiCommand::ActiveSensing => buf.put_u8(0xFE),
            MidiCommand::TuneRequest => buf.put_u8(0xF6),
            MidiCommand::SystemExclusive(data) => {
                buf.put_u8(0xF0);
                buf.put_slice(data);
                buf.put_u8(0xF7);
            }
            MidiCommand::Unknown { status, data } => {
                buf.put_u8(*status);
                buf.put_slice(data);
            }
        }
    }
}

/// Parses raw MIDI bytes into a MidiCommand enum.
/// This function is designed to parse a single MIDI message from a byte slice.
/// It does NOT handle running status or system exclusive messages that span multiple packets.
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
    let mut command_reader = Bytes::from(command_slice);
    let command = MidiCommand::parse(&mut command_reader)?;
    Ok((command, command_length))
}

// Helper to determine MIDI command length (including status byte)
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
                0xF1 => Ok(2), // MIDI Time Code Quarter Frame
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