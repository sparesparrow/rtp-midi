// rtp_midi_lib/src/event_bus.rs

use utils::Event;
use tokio::sync::broadcast;
use anyhow::Result;

pub fn create_event_bus() -> (broadcast::Sender<Event>, broadcast::Receiver<Event>) {
    broadcast::channel::<Event>(16)
} 