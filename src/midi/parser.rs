use anyhow::{anyhow, Result};
use log::warn;

/// Represents a MIDI command.
#[derive(Debug, Clone, PartialEq)]
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
    SystemExclusive(Vec<u8>),
    // Add more MIDI commands as needed
    Unknown { status: u8, data: Vec<u8> },
}

/// Parses raw MIDI bytes into a MidiCommand enum.
/// This function is designed to parse a single MIDI message from a byte slice.
/// It does NOT handle running status or system exclusive messages that span multiple packets.
pub fn parse_midi_message(data: &[u8]) -> Result<(MidiCommand, usize)> {
    if data.is_empty() {
        return Err(anyhow!("Empty MIDI data"));
    }

    let status_byte = data[0];
    let mut bytes_read = 1;

    // Check for running status (data byte without preceding status byte)
    // This simplified parser assumes a status byte is always present for now.
    // A more robust parser would maintain the last seen status byte.
    if status_byte < 0x80 { // It's a data byte, implying running status
        return Err(anyhow!("Running status not supported in this parser (yet)."));
    }

    match status_byte & 0xF0 {
        0x80 => {
            if data.len() < 3 {
                return Err(anyhow!("Incomplete Note Off message"));
            }
            bytes_read += 2;
            Ok((
                MidiCommand::NoteOff {
                    channel: status_byte & 0x0F,
                    key: data[1],
                    velocity: data[2],
                },
                bytes_read,
            ))
        }
        0x90 => {
            if data.len() < 3 {
                return Err(anyhow!("Incomplete Note On message"));
            }
            bytes_read += 2;
            Ok((
                MidiCommand::NoteOn {
                    channel: status_byte & 0x0F,
                    key: data[1],
                    velocity: data[2],
                },
                bytes_read,
            ))
        }
        0xA0 => {
            if data.len() < 3 {
                return Err(anyhow!("Incomplete Polyphonic Key Pressure message"));
            }
            bytes_read += 2;
            Ok((
                MidiCommand::PolyphonicKeyPressure {
                    channel: status_byte & 0x0F,
                    key: data[1],
                    value: data[2],
                },
                bytes_read,
            ))
        }
        0xB0 => {
            if data.len() < 3 {
                return Err(anyhow!("Incomplete Control Change message"));
            }
            bytes_read += 2;
            Ok((
                MidiCommand::ControlChange {
                    channel: status_byte & 0x0F,
                    control: data[1],
                    value: data[2],
                },
                bytes_read,
            ))
        }
        0xC0 => {
            if data.len() < 2 {
                return Err(anyhow!("Incomplete Program Change message"));
            }
            bytes_read += 1;
            Ok((
                MidiCommand::ProgramChange {
                    channel: status_byte & 0x0F,
                    program: data[1],
                },
                bytes_read,
            ))
        }
        0xD0 => {
            if data.len() < 2 {
                return Err(anyhow!("Incomplete Channel Pressure message"));
            }
            bytes_read += 1;
            Ok((
                MidiCommand::ChannelPressure {
                    channel: status_byte & 0x0F,
                    value: data[1],
                },
                bytes_read,
            ))
        }
        0xE0 => {
            if data.len() < 3 {
                return Err(anyhow!("Incomplete Pitch Bend Change message"));
            }
            bytes_read += 2;
            Ok((
                MidiCommand::PitchBendChange {
                    channel: status_byte & 0x0F,
                    value: ((data[2] as u16) << 7) | (data[1] as u16),
                },
                bytes_read,
            ))
        }
        0xF0 => {
            match status_byte {
                0xF0 => {
                    // SysEx Start - read until 0xF7 (End of SysEx)
                    // This parser is simplified and expects complete messages.
                    // For streaming SysEx, a more complex stateful parser is needed.
                    let mut sysex_end = 0;
                    for (i, &byte) in data[1..].iter().enumerate() {
                        if byte == 0xF7 {
                            sysex_end = i + 1; // +1 for the status byte
                            break;
                        }
                    }
                    if sysex_end == 0 {
                        return Err(anyhow!("Incomplete SysEx message: Missing 0xF7"));
                    }
                    bytes_read += sysex_end;
                    Ok((MidiCommand::SystemExclusive(data[1..sysex_end].to_vec()), bytes_read))
                }
                0xF1 => Ok((MidiCommand::Unknown { status: status_byte, data: data[1..].to_vec() }, 2)), // MTC Quarter Frame (1 data byte)
                0xF2 => Ok((MidiCommand::Unknown { status: status_byte, data: data[1..].to_vec() }, 3)), // Song Position Pointer (2 data bytes)
                0xF3 => Ok((MidiCommand::Unknown { status: status_byte, data: data[1..].to_vec() }, 2)), // Song Select (1 data byte)
                0xF6 => Ok((MidiCommand::TuneRequest, 1)),
                0xF8 => Ok((MidiCommand::TimingClock, 1)),
                0xFA => Ok((MidiCommand::Start, 1)),
                0xFB => Ok((MidiCommand::Continue, 1)),
                0xFC => Ok((MidiCommand::Stop, 1)),
                0xFE => Ok((MidiCommand::ActiveSensing, 1)),
                0xFF => Ok((MidiCommand::Unknown { status: status_byte, data: data[1..].to_vec() }, 1)), // Reset or other meta
                _ => Err(anyhow!("Unknown System Common/Real-Time message: 0x{:X}", status_byte)),
            }
        }
        _ => Err(anyhow!("Unknown MIDI status byte: 0x{:X}", status_byte)),
    }
}

// Helper to determine MIDI command length (including status byte)
pub fn midi_command_length(status_byte: u8) -> usize {
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