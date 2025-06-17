use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use log::warn;
use serde::{Deserialize, Serialize};

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
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: true,
            payload_type: 97, // Dynamic Payload Type for MIDI
            sequence_number: 0,
            timestamp: 0,
            ssrc: rand::random::<u32>(), // Random SSRC
            journal_present: false,
            midi_commands: midi_messages,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = Bytes::from(data);
        if reader.len() < 12 {
            return Err(anyhow!("RTP header too short"));
        }

        let byte0 = reader.get_u8();
        let version = (byte0 >> 6) & 0x03;
        let padding = (byte0 >> 5) & 0x01 == 1;
        let extension = (byte0 >> 4) & 0x01 == 1;
        let csrc_count = byte0 & 0x0F;

        if version != 2 {
            return Err(anyhow!("Unsupported RTP version: {}", version));
        }

        let byte1 = reader.get_u8();
        let marker = (byte1 >> 7) & 0x01 == 1;
        let payload_type = byte1 & 0x7F;

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
        while reader.has_remaining() {
            // Handle potential journal descriptor
            if reader.remaining() >= 1 && (reader.peek_u8() == 0x01 || reader.peek_u8() == 0x02) {
                // Journal command (0x01 = MIDI Command, 0x02 = MIDI Command List)
                let journal_descriptor = reader.get_u8();
                let length = reader.get_u8(); // length of the journal entry (excluding itself)
                if reader.remaining() < length as usize {
                    return Err(anyhow!("Incomplete MIDI journal entry"));
                }
                // Skip journal data for now
                reader.advance(length as usize);
                journal_present = true;
                continue;
            }

            // Parse MIDI message delta time (Variable Length Quantity)
            let (delta_time, bytes_read) = parse_variable_length_quantity(reader.chunk())?;
            reader.advance(bytes_read);

            // Parse MIDI command
            if !reader.has_remaining() {
                return Err(anyhow!("Incomplete MIDI command: Missing status byte"));
            }
            let status_byte = reader.get_u8();
            
            let mut command_bytes = vec![status_byte];

            let command_len = midi_command_length(status_byte);
            if reader.remaining() < command_len - 1 { // -1 because status byte is already read
                return Err(anyhow!("Incomplete MIDI command: Expected {} data bytes, got {}", command_len - 1, reader.remaining()));
            }
            for _ in 0..(command_len - 1) {
                command_bytes.push(reader.get_u8());
            }
            midi_commands.push(MidiMessage::new(delta_time, command_bytes));
        }

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
            midi_commands,
        })
    }

    pub fn serialize(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(12 + self.midi_commands.len() * 5); // Rough estimate

        let mut byte0 = (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4) | (self.csrc_count & 0x0F);
        buf.put_u8(byte0);

        let mut byte1 = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
        buf.put_u8(byte1);

        buf.put_u16(self.sequence_number);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.ssrc);

        // TODO: Handle CSRC and Extension headers
        if self.extension {
            warn!("RTP header extension not serialized.");
        }

        for msg in &self.midi_commands {
            // Delta time
            let mut v_buf = [0u8; 4];
            let v_len = encode_variable_length_quantity(msg.delta_time, &mut v_buf)?;
            buf.put(&v_buf[..v_len]);

            // MIDI command
            buf.put(&msg.command[..]);
        }

        Ok(buf.freeze())
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
    let mut value = 0u32;
    let mut bytes_read = 0;
    for &byte in data {
        bytes_read += 1;
        value = (value << 7) | (byte & 0x7F) as u32;
        if (byte & 0x80) == 0 {
            return Ok((value, bytes_read));
        }
        if bytes_read >= 4 { // Max 4 bytes for VLQ
            return Err(anyhow!("Variable Length Quantity too long or malformed"));
        }
    }
    Err(anyhow!("Incomplete Variable Length Quantity"))
}

fn encode_variable_length_quantity(value: u32, buf: &mut [u8; 4]) -> Result<usize> {
    if value == 0 {
        buf[0] = 0;
        return Ok(1);
    }
    let mut tmp = value;
    let mut len = 0;
    while tmp > 0 {
        buf[3 - len] = (tmp & 0x7F) as u8;
        if len < 3 { buf[3 - len] |= 0x80; }
        tmp >>= 7;
        len += 1;
    }
    buf.copy_within((4 - len)..4, 0);
    Ok(len)
}

// Helper to determine MIDI command length
fn midi_command_length(status_byte: u8) -> usize {
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