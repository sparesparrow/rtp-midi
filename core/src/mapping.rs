use utils::{InputEvent, WledOutputAction, Mapping};
use network::midi::parser::MidiCommand;
// All Mapping methods must now be implemented in utils.

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