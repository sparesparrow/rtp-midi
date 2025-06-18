// src/midi/rtp/session.rs

use anyhow::{anyhow, Result};
use log::{error, info, warn};
use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use rtp_midi_core::{DataStreamNetSender, DataStreamNetReceiver, StreamError, JournalData, JournalEntry};
use async_trait::async_trait;

use super::message::{MidiMessage, RtpMidiPacket};
use utils::parse_rtp_packet;

// The listener now receives only the MIDI commands, not the whole packet, for cleaner separation.
type MidiCommandListener = Arc<Mutex<dyn Fn(Vec<MidiMessage>) + Send + Sync>>;
type OutgoingPacketHandler = Arc<Mutex<dyn Fn(String, u16, Vec<u8>) + Send + Sync>>;

const HISTORY_SIZE: usize = 64; // How many sent packets to keep in the journal buffer
const MIDI_CLOCK_HZ: f64 = 31250.0; // Standard MIDI clock frequency in Hz

/// Represents a full RTP-MIDI session, managing sending, receiving, and journaling.
pub struct RtpMidiSession {
    name: String,
    // socket: Arc<UdpSocket>, // Removed: network I/O is now event-driven
    midi_command_listener: Arc<Mutex<Option<MidiCommandListener>>>,
    outgoing_packet_handler: Arc<Mutex<Option<OutgoingPacketHandler>>>,

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
    // Timestamp of the last sent packet, for calculating delta-time
    last_sent_timestamp: Arc<Mutex<u32>>,
}

impl RtpMidiSession {
    /// Creates a new RTP-MIDI session without binding to a socket. Network I/O is external.
    pub async fn new(name: String, _port: u16) -> Result<Self> {
        // The port parameter is now advisory; actual binding is done by network_interface.
        info!("RTP-MIDI session '{}' created.", name);

        Ok(Self {
            name,
            midi_command_listener: Arc::new(Mutex::new(None)),
            outgoing_packet_handler: Arc::new(Mutex::new(None)),
            ssrc: rand::random(),
            sequence_number: Arc::new(Mutex::new(rand::random())),
            send_history: Arc::new(Mutex::new(VecDeque::with_capacity(HISTORY_SIZE))),
            receive_history: Arc::new(Mutex::new(BTreeSet::new())),
            peer_addr: Arc::new(Mutex::new(None)),
            last_sent_timestamp: Arc::new(Mutex::new(0)),
        })
    }

    /// Handles an incoming raw UDP packet, parses it, and processes MIDI commands or journal data.
    pub async fn handle_incoming_packet(&mut self, data: Vec<u8>) {
        let parsed_rtp = match parse_rtp_packet(&data) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse raw RTP packet: {}", e);
                return;
            }
        };

        // Now parse the RTP-MIDI specific payload
        match RtpMidiPacket::parse_midi_payload(&parsed_rtp) {
            Ok(packet) => {
                let mut history = self.receive_history.lock().await;

                // --- Journal Processing ---
                if let Some(journal) = &packet.journal_data {
                    self.process_journal(journal, &mut history);
                }

                // --- Packet Processing ---
                // Check if we've already processed this packet (from a journal).
                if !history.contains(&packet.sequence_number) {
                    history.insert(packet.sequence_number);
                    if !packet.midi_commands.is_empty() {
                        if let Some(cb) = &*self.midi_command_listener.lock().await {
                            let cb_clone = cb.clone();
                            // Pass only the MIDI commands to the listener.
                            cb_clone.lock().await(packet.midi_commands);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse RTP-MIDI payload: {}", e);
            }
        }
    }

    /// Connects the session to a specific remote peer. All sent packets will go here.
    pub async fn connect(&self, peer_addr_str: &str) -> Result<()> {
        let mut peer_addr_lock = self.peer_addr.lock().await;
        let addr = tokio::net::lookup_host(peer_addr_str)
            .await?
            .next()
            .ok_or_else(|| anyhow!("Could not resolve peer address: {}", peer_addr_str))?;
        *peer_addr_lock = Some(addr);
        info!("Session '{}' connected to peer {}", self.name, addr);
        Ok(())
    }

    /// Adds a listener that will be called with MIDI commands from valid, non-duplicate packets.
    pub async fn add_midi_command_handler<F>(&self, callback: F)
    where
        F: Fn(Vec<MidiMessage>) + Send + Sync + 'static,
    {
        let mut listener_lock = self.midi_command_listener.lock().await;
        *listener_lock = Some(Arc::new(Mutex::new(callback)));
    }

    /// Adds a handler that will be called when an outgoing RTP-MIDI packet needs to be sent.
    pub async fn add_outgoing_packet_handler<F>(&self, callback: F)
    where
        F: Fn(String, u16, Vec<u8>) + Send + Sync + 'static,
    {
        let mut handler_lock = self.outgoing_packet_handler.lock().await;
        *handler_lock = Some(Arc::new(Mutex::new(callback)));
    }

    /// Sends a list of MIDI commands, publishing the raw packet via the outgoing handler.
    pub async fn send_midi(&self, commands: Vec<MidiMessage>) -> Result<()> {
        let peer_guard = self.peer_addr.lock().await;
        let peer = peer_guard.ok_or_else(|| anyhow!("Cannot send MIDI, no peer connected."))?;

        let mut seq_num_lock = self.sequence_number.lock().await;
        let mut last_sent_ts_lock = self.last_sent_timestamp.lock().await;
        
        let current_timestamp = (tokio::time::Instant::now().elapsed().as_secs_f64() * MIDI_CLOCK_HZ) as u32;
        let delta_time_is_zero = commands.iter().all(|cmd| cmd.delta_time == 0);
        let is_sysex_start = commands.first().map_or(false, |cmd| cmd.command.first().map_or(false, |&b| b == 0xF0));

        let mut packet = RtpMidiPacket::new(self.ssrc, *seq_num_lock, current_timestamp);
        packet.midi_commands = commands.clone();
        packet.delta_time_is_zero = delta_time_is_zero;
        packet.is_sysex_start = is_sysex_start;

        // Add journal from our send history (Enhanced Journal).
        let history_lock = self.send_history.lock().await;
        if !history_lock.is_empty() {
            let checkpoint_seq = history_lock.front().map_or(0, |entry| entry.sequence_nr);
            packet.journal_data = Some(JournalData::Enhanced {
                a_bit: false,
                ch_bits: 0,
                checkpoint_sequence_number: checkpoint_seq as u8,
                entries: history_lock.iter().cloned().collect(),
            });
        }
        drop(history_lock);

        // Serialize the packet.
        let buffer = packet.serialize()?;

        // Send the packet via the outgoing handler (event bus).
        if let Some(handler) = &*self.outgoing_packet_handler.lock().await {
            handler.lock().await(peer.ip().to_string(), peer.port(), buffer.to_vec());
        } else {
            warn!("No outgoing packet handler registered. Packet not sent.");
        }

        // Update last sent timestamp
        *last_sent_ts_lock = current_timestamp;

        // Add this packet to our send history for future journals.
        let mut history_lock = self.send_history.lock().await;
        if history_lock.len() == HISTORY_SIZE {
            history_lock.pop_front(); // Keep the history buffer from growing indefinitely.
        }
        history_lock.push_back(JournalEntry {
            sequence_nr: packet.sequence_number,
            commands: Vec::new(), // TODO: Map MidiMessage to TimedMidiCommand
        });

        // Increment sequence number for the next packet.
        *seq_num_lock = seq_num_lock.wrapping_add(1);

        Ok(())
    }

    /// Processes a journal from an incoming packet, identifies missing packets,
    /// and updates the receive history.
    fn process_journal(&self, journal_data: &JournalData, history: &mut BTreeSet<u16>) {
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
}

#[async_trait::async_trait]
impl DataStreamNetSender for RtpMidiSession {
    fn init(&mut self) -> Result<(), StreamError> {
        // Zde lze inicializovat síťové zdroje, pokud je potřeba
        Ok(())
    }
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        // Odeslání raw MIDI packetu (payload) s timestampem ts
        // Zde by bylo potřeba převést payload na MidiMessage a zavolat send_midi
        // Pro jednoduchost předpokládáme, že payload je již správně připravený
        // (v praxi by zde byla serializace/deserializace)
        // TODO: Implementace podle konkrétního formátu
        Ok(())
    }
}

#[async_trait::async_trait]
impl DataStreamNetReceiver for RtpMidiSession {
    fn init(&mut self) -> Result<(), StreamError> {
        Ok(())
    }
    fn poll(&mut self, buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError> {
        // Zde by se zpracovával příchozí packet a naplnil buf
        // Prozatím pouze stub
        Ok(None)
    }
}
