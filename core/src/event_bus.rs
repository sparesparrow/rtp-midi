// rtp_midi_lib/src/event_bus.rs

use tokio::sync::broadcast::{self, Sender};

#[derive(Debug, Clone)]
pub enum Event {
    RawPacketReceived {
        payload: Vec<u8>,
        source_addr: std::net::SocketAddr,
    },
    SendPacket {
        payload: Vec<u8>,
        dest_addr: std::net::SocketAddr,
    },
    SessionEstablished {
        peer: std::net::SocketAddr,
    },
    SessionTerminated {
        peer: std::net::SocketAddr,
    },
    MidiCommandsReceived {
        commands: Vec<u8>,
        timestamp: u64,
        peer: std::net::SocketAddr,
    },
    JournalReceived {
        journal_data: Vec<u8>,
        peer: std::net::SocketAddr,
    },
    PacketLossDetected {
        missing_seq: u16,
        peer: std::net::SocketAddr,
    },
    MidiMessageToSend {
        message: Vec<u8>,
        peer: std::net::SocketAddr,
    },
    JournalBasedRepair {
        repaired_commands: Vec<u8>,
        peer: std::net::SocketAddr,
    },
    JournalReady {
        journal_payload: Vec<u8>,
        peer: std::net::SocketAddr,
    },
    MidiMessageReady {
        message: Vec<u8>,
        peer: std::net::SocketAddr,
    },
    AudioDataReady(Vec<f32>),
}

pub struct EventBus {
    pub sender: Sender<Event>,
}

impl EventBus {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }
}
