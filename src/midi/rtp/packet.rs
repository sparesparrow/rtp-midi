use anyhow::Result;
use bytes::{Bytes, BytesMut};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::midi::message::MidiMessage;

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
        let midi_data = &data[header_size + 1..];
        let mut midi_commands = Vec::new();
        
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
