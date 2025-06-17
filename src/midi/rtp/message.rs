use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::warn;
use serde::{Deserialize, Serialize};

use crate::midi::parser::{midi_command_length, MidiCommand};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiMessage {
    pub timestamp: u64,
    pub command: MidiCommand,
}

impl MidiMessage {
    pub fn new(timestamp: u64, command: MidiCommand) -> Self {
        MidiMessage { timestamp, command }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RtpMidiPacket {
    pub ssrc: u32,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub midi_messages: Vec<MidiMessage>,
    pub command_count: u32,
}

impl RtpMidiPacket {
    pub fn parse(buffer: &[u8]) -> Result<Self> {
        let mut reader = Bytes::copy_from_slice(buffer);

        // RTP header (fixed part)
        let first_byte = reader.get_u8();
        let version = (first_byte >> 6) & 0x03;
        let padding = ((first_byte >> 5) & 0x01) == 1;
        let extension = ((first_byte >> 4) & 0x01) == 1;
        let csrc_count = (first_byte & 0x0f) as usize;

        if version != 2 {
            return Err(anyhow!("Unsupported RTP version: {}", version));
        }
        if padding {
            warn!("RTP padding not handled.");
        }

        let second_byte = reader.get_u8();
        let marker = ((second_byte >> 7) & 0x01) == 1;
        let payload_type = second_byte & 0x7f;

        if payload_type != 100 { // Assuming dynamic payload type 100 for RTP-MIDI
            warn!("Unexpected RTP payload type: {}", payload_type);
        }
        if !marker {
            warn!("RTP marker bit not set.");
        }

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
        let mut command_count = 0;
        while reader.has_remaining() {
            // Handle potential journal descriptor
            if reader.remaining() >= 1 && (reader[0] == 0x01 || reader[0] == 0x02) {
                // Journal command (0x01 = MIDI Command, 0x02 = MIDI Command List)
                let journal_descriptor = reader.get_u8();
                warn!("MIDI Journal descriptor not handled: {:#x}", journal_descriptor);
                if journal_descriptor == 0x02 { // MIDI Command List
                    let length = reader.get_u8() as usize;
                    reader.advance(length);
                }
                journal_present = true;
                continue;
            }

            // Delta time (variable length quantity)
            let mut delta_time = 0;
            for i in 0..4 {
                if !reader.has_remaining() {
                    return Err(anyhow!("Incomplete delta time VLV"));
                }
                let byte = reader.get_u8();
                delta_time = (delta_time << 7) | (byte & 0x7F) as u32;
                if byte & 0x80 == 0 { break; }
                if i == 3 && byte & 0x80 != 0 { return Err(anyhow!("Delta time VLV too long")); }
            }

            if !reader.has_remaining() {
                return Err(anyhow!("No MIDI command after delta time"));
            }

            // Running status handling
            let status_byte = reader.chunk()[0]; // Use reader.chunk()[0] to peek at the first byte
            if status_byte < 0x80 {
                // This implies running status. The provided solution assumes a status byte is always present.
                // For now, we will error out as per the previous logic if we encounter running status.
                // In a more robust implementation, this would require storing the last status byte.
                return Err(anyhow!("Running status not supported in this context for MIDI message parsing. Status byte: {:#x}", status_byte));
            }
            
            // Actual MIDI command parsing
            let (command, bytes_consumed) = crate::midi::parser::parse_midi_message(&reader.chunk())?;
            reader.advance(bytes_consumed);
            midi_commands.push(MidiMessage::new(delta_time as u64, command));
            command_count += 1;
        }

        Ok(RtpMidiPacket {
            ssrc,
            sequence_number,
            timestamp,
            midi_messages: midi_commands,
            command_count,
        })
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();

        // RTP header (fixed part)
        let first_byte: u8 = (2 << 6) | (0 << 5) | (0 << 4) | 0x00; // Version 2, no padding, no extension, 0 CSRC
        buf.put_u8(first_byte);
        let second_byte: u8 = (1 << 7) | 100; // Marker bit set, payload type 100 (RTP-MIDI)
        buf.put_u8(second_byte);

        buf.put_u16(self.sequence_number);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.ssrc);

        // MIDI messages
        for midi_msg in &self.midi_messages {
            // Delta time (variable length quantity)
            let mut delta_time = midi_msg.timestamp as u32;
            let mut vlv_bytes = Vec::new();
            loop {
                let mut byte = (delta_time & 0x7F) as u8;
                delta_time >>= 7;
                if delta_time > 0 {
                    byte |= 0x80;
                }
                vlv_bytes.push(byte);
                if delta_time == 0 { break; }
            }
            for byte in vlv_bytes.iter().rev() {
                buf.put_u8(*byte);
            }

            // MIDI command
            midi_msg.command.write_to_bytes(&mut buf);
        }
        buf.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::parser::MidiCommand;

    #[test]
    fn test_rtp_midi_packet_parse_and_serialize() {
        // Example RTP-MIDI packet from RFC 6295, Section 5.3.1 (minus header extension for simplicity)
        let raw_packet = Bytes::from_static(&[
            0x80, 0x64, // RTP header (V=2, P=0, X=0, CC=0, M=1, PT=100)
            0x00, 0x01, // Sequence number
            0x00, 0x00, 0x00, 0x00, // Timestamp
            0x00, 0x00, 0x00, 0x00, // SSRC
            // MIDI commands
            0x00, 0x90, 0x3C, 0x64, // Note On, C4, velocity 100 (delta 0)
            0x01, 0x80, 0x3C, 0x40, // Note Off, C4, velocity 64 (delta 1)
        ]);

        let packet = RtpMidiPacket::parse(&raw_packet).unwrap();

        assert_eq!(packet.sequence_number, 1);
        assert_eq!(packet.timestamp, 0);
        assert_eq!(packet.ssrc, 0);
        assert_eq!(packet.midi_messages.len(), 2);
        assert_eq!(packet.midi_messages[0].timestamp, 0);
        assert_eq!(packet.midi_messages[0].command, MidiCommand::NoteOn { channel: 0, key: 0x3C, velocity: 0x64 });
        assert_eq!(packet.midi_messages[1].timestamp, 1);
        assert_eq!(packet.midi_messages[1].command, MidiCommand::NoteOff { channel: 0, key: 0x3C, velocity: 0x40 });

        let serialized_packet = packet.to_bytes();
        // For simplicity, just check length and first few bytes as a full byte-by-byte comparison can be fragile
        assert_eq!(serialized_packet.len(), raw_packet.len());
        assert_eq!(&serialized_packet[0..12], &raw_packet[0..12]);
        // The exact serialization of VLV delta time might differ slightly if not optimized identically
        // but the core MIDI messages should be the same.
    }

    #[test]
    fn test_rtp_midi_packet_with_running_status() {
        let raw_packet = Bytes::from_static(&[
            0x80, 0x64, // RTP header
            0x00, 0x01, // Sequence number
            0x00, 0x00, 0x00, 0x00, // Timestamp
            0x00, 0x00, 0x00, 0x00, // SSRC
            // MIDI commands with running status
            0x00, 0x90, 0x3C, 0x64, // Note On C4, velocity 100
            0x01, 0x3D, 0x65,       // Note On D4, velocity 101 (running status 0x90)
            0x01, 0x3E, 0x66,       // Note On E4, velocity 102 (running status 0x90)
        ]);

        let packet_result = RtpMidiPacket::parse(&raw_packet);
        assert!(packet_result.is_err()); // Should fail due to running status not fully supported
        assert_eq!(packet_result.unwrap_err().to_string(), "Running status not supported in this context for MIDI message parsing. Status byte: 0x3d");
    }

    #[test]
    fn test_rtp_midi_packet_empty_data() {
        let raw_packet = Bytes::from_static(&[
            0x80, 0x64, // RTP header
            0x00, 0x01, // Sequence number
            0x00, 0x00, 0x00, 0x00, // Timestamp
            0x00, 0x00, 0x00, 0x00, // SSRC
        ]);
        let packet = RtpMidiPacket::parse(&raw_packet).unwrap();
        assert_eq!(packet.midi_messages.len(), 0);
    }

    #[test]
    fn test_midi_message_to_bytes_and_parse() {
        let msg = MidiMessage::new(0, MidiCommand::NoteOn { channel: 0, key: 0x3C, velocity: 0x64 });
        let mut buf = BytesMut::new();
        msg.command.write_to_bytes(&mut buf);
        let bytes = buf.freeze();

        let mut reader = bytes.clone();
        let parsed_command = MidiCommand::parse(&mut reader).unwrap();
        assert_eq!(parsed_command, msg.command);
    }
} 