use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use utils::{MidiCommand, parse_midi_message, midi_command_length};

// All MIDI command logic is now provided by utils. Add any parser-specific helpers below if needed. 