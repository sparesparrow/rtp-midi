// src/midi/rtp/message.rs

use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use rtp_midi_core::midi_command_length;
use rtp_midi_core::ParsedPacket;
use serde::{Deserialize, Serialize};

/// Represents a single MIDI message with its delta-time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiMessage {
    pub delta_time: u32,
    pub command: Vec<u8>,
}

impl MidiMessage {
    pub fn new(delta_time: u32, command: Vec<u8>) -> Self {
        Self {
            delta_time,
            command,
        }
    }
}

/// Represents a full RTP-MIDI packet, including the recovery journal.
#[derive(Debug, Clone)]
pub struct RtpMidiPacket {
    // --- RTP Header Fields ---
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,

    // --- RTP-MIDI Payload ---
    pub delta_time_is_zero: bool,
    pub is_sysex_start: bool,
    pub midi_commands: Vec<MidiMessage>,
    pub journal_data: Option<rtp_midi_core::JournalData>,
}

impl RtpMidiPacket {
    /// Creates a new RTP-MIDI packet.
    pub fn new(ssrc: u32, sequence_number: u16, timestamp: u32) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            marker: true,     // Typically true for the last packet of a MIDI event
            payload_type: 97, // Dynamic Payload Type for MIDI
            sequence_number,
            timestamp,
            ssrc,
            midi_commands: Vec::new(),
            journal_data: None,
            delta_time_is_zero: false,
            is_sysex_start: false,
        }
    }

    /// Parses the RTP-MIDI specific payload from a ParsedPacket.
    pub fn parse_midi_payload(parsed_rtp: &ParsedPacket) -> Result<Self> {
        let mut reader = Bytes::copy_from_slice(&parsed_rtp.payload);

        // First byte of payload contains flags
        let flags = reader.get_u8();
        let has_journal = (flags & 0b1000_0000) != 0;
        let delta_time_is_zero = (flags & 0b0100_0000) != 0; // 'Y' flag
        let is_sysex_start = (flags & 0b0010_0000) != 0; // 'P' flag
        let command_section_len = flags & 0b0000_1111; // 'L' field

        let mut midi_commands = Vec::new();
        if command_section_len > 0 {
            midi_commands =
                Self::parse_midi_command_section(&mut reader, command_section_len as usize)?;
        }

        let mut journal_data = None;
        if has_journal {
            journal_data = Some(rtp_midi_core::JournalData::parse_enhanced(&mut reader)?);
        }

        Ok(Self {
            version: parsed_rtp.version,
            padding: parsed_rtp.padding,
            extension: parsed_rtp.extension,
            marker: parsed_rtp.marker,
            payload_type: parsed_rtp.payload_type,
            sequence_number: parsed_rtp.sequence_number,
            timestamp: parsed_rtp.timestamp,
            ssrc: parsed_rtp.ssrc,
            midi_commands,
            journal_data,
            delta_time_is_zero,
            is_sysex_start,
        })
    }

    /// Serializes the packet into a byte buffer for sending.
    pub fn serialize(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(128); // Start with a reasonable capacity

        // --- RTP Header ---
        let byte0 =
            (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4); // No CSRC for now
        buf.put_u8(byte0);

        let byte1 = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
        buf.put_u8(byte1);

        buf.put_u16(self.sequence_number);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.ssrc);

        // --- MIDI Payload ---
        let mut midi_payload = BytesMut::new();
        for msg in &self.midi_commands {
            let mut v_buf = [0u8; 4];
            let v_len = encode_variable_length_quantity(msg.delta_time, &mut v_buf)?;
            midi_payload.put(&v_buf[..v_len]);
            midi_payload.put(&msg.command[..]);
        }
        let command_section_len = midi_payload.len();
        if command_section_len > 15 {
            return Err(anyhow!("MIDI command section too long (max 15 bytes)"));
        }

        // --- Journal Payload ---
        let mut journal_payload = BytesMut::new();
        let has_journal_flag = if let Some(journal) = &self.journal_data {
            journal_payload.put_slice(&journal.serialize_enhanced()?);
            0b1000_0000 // has_journal_flag for the main payload flags
        } else {
            0
        };

        // --- Final Assembly ---
        let flags = has_journal_flag
            | ((self.delta_time_is_zero as u8) << 6)
            | ((self.is_sysex_start as u8) << 5)
            | (command_section_len as u8);

        buf.put_u8(flags);
        buf.put(midi_payload);
        buf.put(journal_payload);

        Ok(buf.freeze())
    }

    // --- Private Helper Functions ---

    fn parse_midi_command_section(reader: &mut Bytes, len: usize) -> Result<Vec<MidiMessage>> {
        if reader.remaining() < len {
            return Err(anyhow!("Incomplete MIDI command section"));
        }
        let mut command_reader = reader.split_to(len);
        let mut commands = Vec::new();

        while command_reader.has_remaining() {
            let (delta_time, bytes_read) = parse_variable_length_quantity(command_reader.chunk())?;
            command_reader.advance(bytes_read);

            let status_byte = *command_reader
                .chunk()
                .first()
                .ok_or_else(|| anyhow!("Missing status byte"))?;

            let cmd_len = midi_command_length(status_byte)?;

            if command_reader.remaining() < cmd_len {
                return Err(anyhow!("Incomplete MIDI command data"));
            }
            let command_data = command_reader.copy_to_bytes(cmd_len);
            commands.push(MidiMessage::new(delta_time, command_data.to_vec()));
        }
        Ok(commands)
    }

    pub fn midi_commands(&self) -> &Vec<MidiMessage> {
        &self.midi_commands
    }
}

fn parse_variable_length_quantity(mut data: &[u8]) -> Result<(u32, usize)> {
    let mut value = 0u32;
    let mut length = 0;
    for _ in 0..4 {
        // VLQ is max 4 bytes
        if !data.has_remaining() {
            return Err(anyhow!("Incomplete Variable Length Quantity"));
        }
        let byte = data.get_u8();
        length += 1;
        value = (value << 7) | (byte & 0x7F) as u32;
        if (byte & 0x80) == 0 {
            // Last byte of VLQ
            return Ok((value, length));
        }
    }
    Err(anyhow!(
        "Variable Length Quantity exceeded 4 bytes or malformed"
    ))
}

fn encode_variable_length_quantity(value: u32, buf: &mut [u8; 4]) -> Result<usize> {
    if value == 0 {
        buf[0] = 0;
        return Ok(1);
    }

    let mut temp = value;
    let mut idx = 3;
    buf[idx] = (temp & 0x7F) as u8;
    temp >>= 7;

    while temp > 0 {
        idx -= 1;
        buf[idx] = ((temp & 0x7F) | 0x80) as u8;
        temp >>= 7;
    }

    let start = idx;
    let length = 4 - start;
    for i in 0..length {
        buf[i] = buf[start + i];
    }
    Ok(length)
}
