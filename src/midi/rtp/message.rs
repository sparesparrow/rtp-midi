// src/midi/rtp/message.rs

use crate::midi::parser::midi_command_length;
use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
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

/// Represents the data contained within the RTP-MIDI Recovery Journal.
#[derive(Debug, Clone)]
pub enum JournalData {
    /// Enhanced Recovery Journal as defined in RFC 6295, Section 6.2.2
    Enhanced {
        /// 0 for MIDI channel journal, 1 for system common/real-time journal.
        a_bit: bool,
        /// Channel number for channel journals (0-63), or 0 for system journals.
        ch_bits: u8,
        /// The sequence number of the checkpoint packet.
        checkpoint_sequence_number: u8,
        /// The list of journal entries.
        entries: Vec<JournalEntry>,
    },
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
    pub journal_data: Option<JournalData>,
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
            journal_data: None,
            delta_time_is_zero: false,
            is_sysex_start: false,
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
                journal_data: None,
                delta_time_is_zero: false,
                is_sysex_start: false,
            });
        }
        
        let mut midi_commands = Vec::new();
        let mut journal_data = None;
        
        // First byte of payload contains flags
        let flags = reader.get_u8();
        let has_journal = (flags & 0b1000_0000) != 0;
        let delta_time_is_zero = (flags & 0b0100_0000) != 0; // 'Y' flag
        let is_sysex_start = (flags & 0b0010_0000) != 0;     // 'P' flag
        let command_section_len = flags & 0b0000_1111; // 'L' field

        if command_section_len > 0 {
             midi_commands = Self::parse_midi_command_section(&mut reader, command_section_len as usize)?;
        }
        
        if has_journal {
            journal_data = Some(Self::parse_enhanced_journal_section(&mut reader)?);
        }
        
        Ok(Self {
            version, padding, extension, marker, payload_type,
            sequence_number, timestamp, ssrc,
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
        let has_journal_flag = match &self.journal_data {
            Some(JournalData::Enhanced { a_bit, ch_bits, checkpoint_sequence_number, entries }) => {
                // Enhanced Journal Header (RFC6295 Section 6.2.2)
                let s_bit = 0b1000_0000; // S=1 (enhanced)
                let a_ch_bits = ((*a_bit as u8) << 6) | (*ch_bits & 0b0011_1111);
                journal_payload.put_u8(s_bit | a_ch_bits);
                journal_payload.put_u8(*checkpoint_sequence_number);
                journal_payload.put_u16(entries.len() as u16); // Count of packets in journal
                
                for entry in entries {
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
                0b1000_0000 // has_journal_flag for the main payload flags
            },
            None => 0,
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
    
    fn parse_enhanced_journal_section(reader: &mut Bytes) -> Result<JournalData> {
        if reader.remaining() < 4 {
             return Err(anyhow!("Incomplete enhanced journal header"));
        }
        let header_byte0 = reader.get_u8();
        let s_bit = (header_byte0 >> 7) & 0x01 == 1; // Should always be 1 for enhanced
        let a_bit = (header_byte0 >> 6) & 0x01 == 1;
        let ch_bits = header_byte0 & 0x3F;

        if !s_bit {
            return Err(anyhow!("Unsupported journal type: not an enhanced journal"));
        }

        let checkpoint_sequence_number = reader.get_u8();
        let packet_count = reader.get_u16();
        
        let mut entries = Vec::with_capacity(packet_count as usize);
        for _ in 0..packet_count {
             if reader.remaining() < 4 {
                 return Err(anyhow!("Incomplete journal entry header"));
             }
             let sequence_nr = reader.get_u16();
             let length = reader.get_u16();
             let commands = Self::parse_midi_command_section(reader, length as usize)?;
             entries.push(JournalEntry { sequence_nr, commands });
        }
        
        Ok(JournalData::Enhanced {
            a_bit,
            ch_bits,
            checkpoint_sequence_number,
            entries,
        })
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

