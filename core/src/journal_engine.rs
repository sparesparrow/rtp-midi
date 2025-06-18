// rtp_midi_lib/src/journal_engine.rs

use anyhow::{anyhow, Result};
use std::collections::BTreeSet;

use utils::{midi_command_length, MidiCommand};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// MIDI příkaz s časováním (delta_time)
#[derive(Debug, Clone, PartialEq)]
pub struct TimedMidiCommand {
    pub delta_time: u32,
    pub command: MidiCommand,
}

/// Represents a history entry in the recovery journal.
#[derive(Debug, Clone, PartialEq)]
pub struct JournalEntry {
    pub sequence_nr: u16,
    pub commands: Vec<TimedMidiCommand>,
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

impl JournalEntry {
    pub fn serialize(&self) -> Result<Bytes> {
        let mut buf = BytesMut::new();
        // Sequence Number (2 bytes)
        buf.put_u16(self.sequence_nr);

        // Timed MIDI Commands (Variable Length)
        for msg in &self.commands {
            let mut v_buf = [0u8; 4];
            let v_len = encode_variable_length_quantity(msg.delta_time, &mut v_buf)?;
            buf.put(&v_buf[..v_len]);
            // Serializace samotného MIDI příkazu do bytu
            let mut midi_bytes = Vec::new();
            serialize_midi_command(&msg.command, &mut midi_bytes)?;
            buf.put(&midi_bytes[..]);
        }
        Ok(buf.freeze())
    }

    pub fn parse(data: &mut Bytes) -> Result<Self> {
        if data.len() < 2 {
            return Err(anyhow!("Journal entry too short for sequence number"));
        }
        let sequence_nr = data.get_u16();

        let mut commands = Vec::new();
        while data.has_remaining() {
            let (delta_time, delta_len) = parse_variable_length_quantity(data)?;
            data.advance(delta_len); // Consume VLQ bytes

            if !data.has_remaining() {
                break;
            }
            let status_byte = data[0];
            let command_len = midi_command_length(status_byte)?;
            if data.remaining() < command_len {
                return Err(anyhow!("Not enough data for MIDI command in journal entry"));
            }
            let command_bytes = data.copy_to_bytes(command_len);
            let mut command_reader = command_bytes.clone();
            let command = MidiCommand::parse(&mut command_reader)?;
            commands.push(TimedMidiCommand { delta_time, command });
        }

        Ok(Self { sequence_nr, commands })
    }
}

impl JournalData {
    pub fn serialize_enhanced(&self) -> Result<Bytes> {
        if let JournalData::Enhanced { a_bit, ch_bits, checkpoint_sequence_number, entries } = self {
            let mut buf = BytesMut::new();
            let s_bit = 0b1000_0000; // S=1 (enhanced)
            let a_ch_bits = ((*a_bit as u8) << 6) | (*ch_bits & 0b0011_1111);
            buf.put_u8(s_bit | a_ch_bits);
            buf.put_u8(*checkpoint_sequence_number);
            buf.put_u16(entries.len() as u16); // Count of packets in journal

            for entry in entries {
                buf.put_slice(&entry.serialize()?);
            }
            Ok(buf.freeze())
        } else {
            Err(anyhow!("Only Enhanced JournalData can be serialized currently"))
        }
    }

    pub fn parse_enhanced(data: &mut Bytes) -> Result<Self> {
        if data.len() < 4 {
            return Err(anyhow!("Enhanced Journal header too short"));
        }

        let byte0 = data.get_u8();
        let s_bit = (byte0 >> 7) & 0x01 == 1;
        if !s_bit {
            return Err(anyhow!("Not an Enhanced Journal (S-bit is 0)"));
        }

        let a_bit = (byte0 >> 6) & 0x01 == 1;
        let ch_bits = byte0 & 0b0011_1111;
        let checkpoint_sequence_number = data.get_u8();
        let entry_count = data.get_u16() as usize;

        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(JournalEntry::parse(data)?);
        }

        Ok(JournalData::Enhanced {
            a_bit,
            ch_bits,
            checkpoint_sequence_number,
            entries,
        })
    }
}

pub fn process_journal(journal_data: &JournalData, history: &mut BTreeSet<u16>) {
    let entries = match journal_data {
        JournalData::Enhanced { entries, .. } => entries,
    };

    if entries.is_empty() {
        return;
    }

    // For simplicity, we are not requesting retransmissions yet, but just updating history.
    for entry in entries {
        history.insert(entry.sequence_nr);
    }
}

// Helper functions (could be moved to rtp_midi_utils if truly generic)

fn parse_variable_length_quantity(data: &mut Bytes) -> Result<(u32, usize)> {
    let mut value = 0u32;
    let mut length = 0;
    for _ in 0..4 { // VLQ is max 4 bytes
        if !data.has_remaining() {
            return Err(anyhow!("Incomplete Variable Length Quantity"));
        }
        let byte = data.get_u8();
        length += 1;
        value = (value << 7) | (byte & 0x7F) as u32;
        if (byte & 0x80) == 0 { // Last byte of VLQ
            return Ok((value, length));
        }
    }
    Err(anyhow!("Variable Length Quantity exceeded 4 bytes or malformed"))
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
        if idx < 0 {
            return Err(anyhow!("VLQ encoding overflow"));
        }
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

// MIDI příkaz na byty (pro serializaci)
fn serialize_midi_command(cmd: &MidiCommand, out: &mut Vec<u8>) -> Result<()> {
    // Pro jednoduchost použijeme existující serializaci do raw MIDI bytu
    // (můžete rozšířit podle potřeby)
    match cmd {
        MidiCommand::NoteOff { channel, key, velocity } => {
            out.push(0x80 | (channel & 0x0F));
            out.push(*key);
            out.push(*velocity);
        }
        MidiCommand::NoteOn { channel, key, velocity } => {
            out.push(0x90 | (channel & 0x0F));
            out.push(*key);
            out.push(*velocity);
        }
        MidiCommand::PolyphonicKeyPressure { channel, key, value } => {
            out.push(0xA0 | (channel & 0x0F));
            out.push(*key);
            out.push(*value);
        }
        MidiCommand::ControlChange { channel, control, value } => {
            out.push(0xB0 | (channel & 0x0F));
            out.push(*control);
            out.push(*value);
        }
        MidiCommand::ProgramChange { channel, program } => {
            out.push(0xC0 | (channel & 0x0F));
            out.push(*program);
        }
        MidiCommand::ChannelPressure { channel, value } => {
            out.push(0xD0 | (channel & 0x0F));
            out.push(*value);
        }
        MidiCommand::PitchBendChange { channel, value } => {
            out.push(0xE0 | (channel & 0x0F));
            out.push((*value & 0x7F) as u8);
            out.push(((*value >> 7) & 0x7F) as u8);
        }
        MidiCommand::TimingClock => out.push(0xF8),
        MidiCommand::Start => out.push(0xFA),
        MidiCommand::Continue => out.push(0xFB),
        MidiCommand::Stop => out.push(0xFC),
        MidiCommand::ActiveSensing => out.push(0xFE),
        MidiCommand::TuneRequest => out.push(0xF6),
        MidiCommand::SystemExclusive(data) => {
            out.push(0xF0);
            out.extend_from_slice(data);
            out.push(0xF7);
        }
        MidiCommand::Unknown { status, data } => {
            out.push(*status);
            out.extend_from_slice(data);
        }
    }
    Ok(())
} 