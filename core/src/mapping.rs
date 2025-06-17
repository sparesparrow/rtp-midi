use serde::{Deserialize, Serialize};
use network::midi::parser::MidiCommand;

/// Enum reprezentující různé typy vstupních událostí, které mohou spustit mapování.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputEvent {
    /// Událost MIDI Note On. Obsahuje volitelnou notu a velocity pro filtrování.
    MidiNoteOn {
        note: Option<u8>,
        velocity: Option<u8>,
    },
    /// Událost MIDI Control Change. Obsahuje volitelný kontroler a hodnotu.
    MidiControlChange {
        controller: Option<u8>,
        value: Option<u8>,
    },
    /// Událost detekce špičky v audio signálu.
    AudioPeak,
    /// Událost spojená s konkrétním frekvenčním pásmem audia.
    AudioBand {
        band: String, // Např. "bass", "mid", "treble"
        threshold: Option<f32>, // Volitelná prahová hodnota pro aktivaci
    },
    Midi(MidiCommand),
}

/// Enum reprezentující různé typy akcí, které lze provést na WLED zařízení.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WledOutputAction {
    /// Nastaví WLED preset.
    SetPreset {
        id: i32,
    },
    /// Nastaví celkový jas WLED zařízení.
    SetBrightness {
        value: u8, // 0-255
    },
    /// Nastaví barvu WLED segmentu (aktuálně jen pro hlavní segment).
    SetColor {
        r: u8,
        g: u8,
        b: u8,
    },
    /// Nastaví efekt WLED segmentu.
    SetEffect {
        id: i32,
        speed: Option<u8>, // 0-255
        intensity: Option<u8>, // 0-255
    },
    /// Nastaví paletu barev WLED segmentu.
    SetPalette {
        id: i32,
    },
}

/// Struktura definující jedno mapování ze vstupní události na WLED akci.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    pub input: InputEvent,
    pub output: Vec<WledOutputAction>,
}

impl Mapping {
    /// Porovná daný MIDI příkaz se vstupní událostí typu MidiNoteOn nebo MidiControlChange.
    pub fn matches_midi_command(&self, command: &MidiCommand) -> bool {
        match (&self.input, command) {
            (InputEvent::MidiNoteOn { note, velocity: note_vel }, MidiCommand::NoteOn { channel: _, key, velocity: cmd_vel }) => {
                (note.is_none() || *note == Some(*key))
                    && (note_vel.is_none() || *note_vel == Some(*cmd_vel))
            },
            (InputEvent::MidiControlChange { controller, value }, MidiCommand::ControlChange { channel: _, control, value: cc_val }) => {
                (controller.is_none() || *controller == Some(*control))
                    && (value.is_none() || *value == Some(*cc_val))
            },
            _ => false,
        }
    }
} 