// rtp_midi_lib/src/event_bus.rs

use tokio::sync::broadcast;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum Event {
    AudioDataReady(Vec<f32>),
    MidiMessageReceived(Vec<u8>),
    RawPacketReceived { source: String, data: Vec<u8> },
    SendPacket { destination: String, port: u16, data: Vec<u8> },
    // Add more event types as needed
}

pub fn create_event_bus() -> (broadcast::Sender<Event>, broadcast::Receiver<Event>) {
    broadcast::channel::<Event>(16)
} 