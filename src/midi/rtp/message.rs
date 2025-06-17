// src/midi/rtp/message.rs

use crate::midi::parser::{midi_command_length, MidiCommand};
use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::warn;
use rand;
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

/// Represents a history entry in the recovery journal.
#[derive(Debug, Clone, PartialEq)]
pub struct JournalEntry {
    pub sequence_nr: u16,
    pub commands: Vec<MidiMessage>,
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
    pub midi_commands: Vec<MidiMessage>,
    pub journal: Vec<JournalEntry>,
}

impl RtpMidiPacket {
    /// Creates a new RTP-MIDI packet.
    pub fn new(ssrc: u32, sequence_number: u16, timestamp: u32) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            marker: true, // Typically true for the last packet of a MIDI event
            payload_type: 97, // Dynamic Payload Type for MIDI
            sequence_number,
            timestamp,
            ssrc,
            midi_commands: Vec::new(),
            journal: Vec::new(),
        }
    }

    /// Parses an RTP-MIDI packet from a byte slice.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = Bytes::copy_from_slice(data);
        if reader.len() < 12 {
            return Err(anyhow!("RTP header too short"));
        }

        // --- Parse RTP Header ---
        let byte0 = reader.get_u8();
        let version = (byte0 >> 6) & 0x03;
        if version != 2 {
            return Err(anyhow!("Unsupported RTP version: {}", version));
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

        // --- Parse MIDI Payload ---
        if !reader.has_remaining() {
            return Ok(Self { // Packet might have an empty payload (e.g. keep-alive)
                version, padding, extension, marker, payload_type, sequence_number, timestamp, ssrc,
                midi_commands: vec![],
                journal: vec![],
            });
        }
        
        let mut midi_commands = Vec::new();
        let mut journal = Vec::new();
        
        // First byte of payload contains flags
        let flags = reader.get_u8();
        let has_journal = (flags & 0b1000_0000) != 0;
        let _delta_time_is_zero = (flags & 0b0100_0000) != 0; // 'Y' flag
        let _is_sysex_start = (flags & 0b0010_0000) != 0;     // 'P' flag
        let command_section_len = flags & 0b0000_1111; // 'L' field

        if command_section_len > 0 {
             midi_commands = Self::parse_midi_command_section(&mut reader, command_section_len as usize)?;
        }
        
        if has_journal {
            journal = Self::parse_journal_section(&mut reader)?;
        }
        
        Ok(Self {
            version, padding, extension, marker, payload_type,
            sequence_number, timestamp, ssrc,
            midi_commands,
            journal,
        })
    }

    /// Serializes the packet into a byte buffer for sending.
    pub fn serialize(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(128); // Start with a reasonable capacity

        // --- RTP Header ---
        let byte0 = (self.version << 6)
            | ((self.padding as u8) << 5)
            | ((self.extension as u8) << 4)
            | 0; // No CSRC for now
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
        if !self.journal.is_empty() {
             // Enhanced Journal Header (RFC6295 Section 6.2.2)
            journal_payload.put_u8(0b1000_0000); // S=1 (enhanced), A=0, CH=0
            journal_payload.put_u8(0x00); // Checkpoint Packet Seqnum (placeholder)
            journal_payload.put_u16(self.journal.len() as u16); // Count of packets in journal
            
            for entry in &self.journal {
                 let mut entry_payload = BytesMut::new();
                 for cmd in &entry.commands {
                    let mut v_buf = [0u8; 4];
                    let v_len = encode_variable_length_quantity(cmd.delta_time, &mut v_buf)?;
                    entry_payload.put(&v_buf[..v_len]);
                    entry_payload.put(&cmd.command[..]);
                 }
                 journal_payload.put_u16(entry.sequence_nr);
                 journal_payload.put_u16(entry_payload.len() as u16);
                 journal_payload.put(entry_payload);
            }
        }
        
        // --- Final Assembly ---
        let has_journal_flag = if !self.journal.is_empty() { 0b1000_0000 } else { 0 };
        let flags = has_journal_flag | (command_section_len as u8);
        
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
            
            let status_byte = *command_reader.chunk().first().ok_or_else(|| anyhow!("Missing status byte"))?;
            
            let cmd_len = midi_command_length(status_byte)?;
            
            if command_reader.remaining() < cmd_len {
                 return Err(anyhow!("Incomplete MIDI command data"));
            }
            let command_data = command_reader.copy_to_bytes(cmd_len);
            commands.push(MidiMessage::new(delta_time, command_data.to_vec()));
        }
        Ok(commands)
    }
    
    fn parse_journal_section(reader: &mut Bytes) -> Result<Vec<JournalEntry>> {
        if reader.remaining() < 4 {
             return Err(anyhow!("Incomplete journal header"));
        }
        let _journal_header = reader.get_u8();
        let _checkpoint_seqnum = reader.get_u8();
        let packet_count = reader.get_u16();
        
        let mut journal = Vec::with_capacity(packet_count as usize);
        for _ in 0..packet_count {
             if reader.remaining() < 4 {
                 return Err(anyhow!("Incomplete journal entry header"));
             }
             let sequence_nr = reader.get_u16();
             let length = reader.get_u16();
             let commands = Self::parse_midi_command_section(reader, length as usize)?;
             journal.push(JournalEntry { sequence_nr, commands });
        }
        
        Ok(journal)
    }

    pub fn midi_commands(&self) -> &Vec<MidiMessage> {
        &self.midi_commands
    }
}

// --- VLQ Encoding/Decoding Helpers ---

fn parse_variable_length_quantity(data: &[u8]) -> Result<(u32, usize)> {
    let mut value = 0u32;
    let mut bytes_read = 0;
    for &byte in data {
        bytes_read += 1;
        value = (value << 7) | (byte & 0x7F) as u32;
        if (byte & 0x80) == 0 {
            return Ok((value, bytes_read));
        }
        if bytes_read >= 4 {
            return Err(anyhow!("Variable Length Quantity too long"));
        }
    }
    Err(anyhow!("Incomplete Variable Length Quantity"))
}

fn encode_variable_length_quantity(value: u32, buf: &mut [u8; 4]) -> Result<usize> {
    if value > 0x0FFFFFFF {
        return Err(anyhow!("Value too large for VLQ encoding"));
    }
    let mut tmp = value;
    let mut len = 0;
    let mut bytes = [0u8; 4];

    loop {
        bytes[len] = (tmp & 0x7F) as u8;
        tmp >>= 7;
        if len > 0 {
            bytes[len] |= 0x80;
        }
        len += 1;
        if tmp == 0 {
            break;
        }
    }
    
    for i in 0..len {
        buf[i] = bytes[len - 1 - i];
    }

    Ok(len)
}

