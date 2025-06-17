use utils::{InputEvent, WledOutputAction, Mapping, MidiCommand};

pub fn matches_midi_command(mapping: &Mapping, command: &MidiCommand) -> bool {
    match (&mapping.input, command) {
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