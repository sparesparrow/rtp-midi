// src/midi/rtp/session.rs

use anyhow::Result;
use log::{error, info, warn};
use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use super::message::{JournalEntry, MidiMessage, RtpMidiPacket};

// The listener now receives only the MIDI commands, not the whole packet, for cleaner separation.
type CommandListener = Arc<Mutex<dyn Fn(Vec<MidiMessage>) + Send + Sync>>;

const HISTORY_SIZE: usize = 64; // How many sent packets to keep in the journal buffer

/// Represents a full RTP-MIDI session, managing sending, receiving, and journaling.
pub struct RtpMidiSession {
    name: String,
    socket: Arc<UdpSocket>,
    listener: Arc<Mutex<Option<CommandListener>>>,

    // --- Session State ---
    ssrc: u32,
    sequence_number: Arc<Mutex<u16>>,

    // --- Journaling State ---
    // History of packets we have sent, to be included in the journal of outgoing packets.
    send_history: Arc<Mutex<VecDeque<JournalEntry>>>,
    // The sequence numbers of packets we have received, to detect gaps.
    receive_history: Arc<Mutex<BTreeSet<u16>>>,

    // --- Peer information ---
    peer_addr: Arc<Mutex<Option<SocketAddr>>>,
}

impl RtpMidiSession {
    /// Creates a new RTP-MIDI session and binds it to a local port.
    pub async fn new(name: String, port: u16) -> Result<Self> {
        let addr = format!("0.0.0.0:{}", port);
        let socket = Arc::new(UdpSocket::bind(&addr).await?);
        info!("RTP-MIDI session '{}' bound to {}", name, addr);

        Ok(Self {
            name,
            socket,
            listener: Arc::new(Mutex::new(None)),
            ssrc: rand::random(),
            sequence_number: Arc::new(Mutex::new(rand::random())),
            send_history: Arc::new(Mutex::new(VecDeque::with_capacity(HISTORY_SIZE))),
            receive_history: Arc::new(Mutex::new(BTreeSet::new())),
            peer_addr: Arc::new(Mutex::new(None)),
        })
    }

    /// Connects the session to a specific remote peer. All sent packets will go here.
    pub async fn connect(&self, peer_addr_str: &str) -> Result<()> {
        let mut peer_addr_lock = self.peer_addr.lock().await;
        let addr = tokio::net::lookup_host(peer_addr_str)
            .await?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve peer address: {}", peer_addr_str))?;
        *peer_addr_lock = Some(addr);
        info!("Session '{}' connected to peer {}", self.name, addr);
        Ok(())
    }

    /// Adds a listener that will be called with MIDI commands from valid, non-duplicate packets.
    pub fn add_listener<F>(&self, callback: F)
    where
        F: Fn(Vec<MidiMessage>) + Send + Sync + 'static,
    {
        let mut listener_lock = self.listener.try_lock().expect("Failed to lock listener");
        *listener_lock = Some(Arc::new(Mutex::new(callback)));
    }

    /// Sends a list of MIDI commands to the connected peer.
    /// This method now creates a packet, populates it with a journal of recent packets,
    /// sends it, and updates its own history.
    pub async fn send_midi(&self, commands: Vec<MidiMessage>) -> Result<()> {
        let peer_guard = self.peer_addr.lock().await;
        let peer = peer_guard.ok_or_else(|| anyhow::anyhow!("Cannot send MIDI, no peer connected."))?;

        let mut seq_num_lock = self.sequence_number.lock().await;
        let timestamp = 0; // Timestamping can be improved later (e.g., based on a media clock).

        let mut packet = RtpMidiPacket::new(self.ssrc, *seq_num_lock, timestamp);
        packet.midi_commands = commands.clone();

        // Add journal from our send history.
        let history_lock = self.send_history.lock().await;
        packet.journal = history_lock.iter().cloned().collect();
        drop(history_lock);

        // Serialize and send the packet.
        let buffer = packet.serialize()?;
        self.socket.send_to(&buffer, peer).await?;

        // Add this packet to our send history for future journals.
        let mut history_lock = self.send_history.lock().await;
        if history_lock.len() == HISTORY_SIZE {
            history_lock.pop_front(); // Keep the history buffer from growing indefinitely.
        }
        history_lock.push_back(JournalEntry {
            sequence_nr: packet.sequence_number,
            commands,
        });

        // Increment sequence number for the next packet.
        *seq_num_lock = seq_num_lock.wrapping_add(1);

        Ok(())
    }

    /// Starts the main loop for listening to incoming packets.
    pub async fn start(&self) -> Result<()> {
        let socket = self.socket.clone();
        let listener = self.listener.clone();
        let receive_history = self.receive_history.clone();
        let mut buf = [0; 1500]; // Buffer for incoming packets.

        info!("Session '{}' started listening for packets.", self.name);

        loop {
            let (len, src_addr) = socket.recv_from(&mut buf).await?;
            let data = &buf[..len];

            // Auto-discover the first peer that sends us a message if not connected.
            let mut peer_lock = self.peer_addr.lock().await;
            if peer_lock.is_none() {
                *peer_lock = Some(src_addr);
                info!("Session '{}' auto-discovered peer {}", self.name, src_addr);
            }
            drop(peer_lock);

            match RtpMidiPacket::parse(data) {
                Ok(packet) => {
                    let mut history = receive_history.lock().await;

                    // --- Journal Processing ---
                    self.process_journal(&packet.journal, &mut history);

                    // --- Packet Processing ---
                    // Check if we've already processed this packet (from a journal).
                    if !history.contains(&packet.sequence_number) {
                        history.insert(packet.sequence_number);
                        if !packet.midi_commands.is_empty() {
                            if let Some(cb) = &*listener.lock().await {
                                let cb_clone = cb.clone();
                                // Pass only the MIDI commands to the listener.
                                cb_clone.lock().await(packet.midi_commands);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse RTP-MIDI packet from {}: {}", src_addr, e);
                }
            }
        }
    }

    /// Processes a journal from an incoming packet, identifies missing packets,
    /// and updates the receive history.
    fn process_journal(&self, journal: &[JournalEntry], history: &mut BTreeSet<u16>) {
        if journal.is_empty() {
            return;
        }

        let first_seq_in_journal = journal.first().unwrap().sequence_nr;
        let last_seq_in_history = history.iter().next_back().cloned().unwrap_or(first_seq_in_journal.saturating_sub(1));

        // Simple gap detection using sequence number wrapping.
        if first_seq_in_journal > last_seq_in_history.wrapping_add(1) {
            warn!(
                "Gap detected in RTP stream. Last received: {}, first in journal: {}. Packets may be lost.",
                last_seq_in_history, first_seq_in_journal
            );
            // TODO: Here you would implement logic to request retransmission
            // of the missing packets based on the journal content.
        }

        // Add all sequence numbers from the journal to our history.
        for entry in journal {
            history.insert(entry.sequence_nr);
        }
    }
}
