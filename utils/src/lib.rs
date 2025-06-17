use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum WledOutputAction {
    SetPreset { id: i32 },
    SetBrightness { value: u8 },
    SetColor { r: u8, g: u8, b: u8 },
    SetEffect { id: i32, speed: Option<u8>, intensity: Option<u8> },
    SetPalette { id: i32 },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Mapping {
    pub input: InputEvent,
    pub output: Vec<WledOutputAction>,
}

impl Mapping {
    pub fn matches_midi_command(&self, command: &crate::MidiCommand) -> bool {
        match (&self.input, command) {
            (InputEvent::MidiNoteOn { note, velocity: note_vel }, crate::MidiCommand::NoteOn { channel: _, key, velocity: cmd_vel }) => {
                (note.is_none() || *note == Some(*key))
                    && (note_vel.is_none() || *note_vel == Some(*cmd_vel))
            },
            (InputEvent::MidiControlChange { controller, value }, crate::MidiCommand::ControlChange { channel: _, control, value: cc_val }) => {
                (controller.is_none() || *controller == Some(*control))
                    && (value.is_none() || *value == Some(*cc_val))
            },
            _ => false,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
