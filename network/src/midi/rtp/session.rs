#![deny(warnings)]
// src/midi/rtp/session.rs

use anyhow::{anyhow, Result};
use log::{error, info, warn};
use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use rtp_midi_core::{DataStreamNetSender, DataStreamNetReceiver, StreamError, JournalData, JournalEntry};
use rtp_midi_core::journal_engine::TimedMidiCommand;
use rtp_midi_core::event_bus::Event;

use super::message::{MidiMessage, RtpMidiPacket};
use rtp_midi_core::parse_rtp_packet;
use super::control_message::{AppleMidiMessage, Invitation, InvitationAccepted, Sync as AppleMidiSync};
use tokio::time::{Duration, Sleep, sleep};
use std::pin::Pin;

// The listener now receives only the MIDI commands, not the whole packet, for cleaner separation.
type MidiCommandListener = Arc<Mutex<dyn Fn(Vec<MidiMessage>) + Send + Sync>>;
type OutgoingPacketHandler = Arc<Mutex<dyn Fn(String, u16, Vec<u8>) + Send + Sync>>;

const HISTORY_SIZE: usize = 64; // How many sent packets to keep in the journal buffer
const MIDI_CLOCK_HZ: f64 = 31250.0; // Standard MIDI clock frequency in Hz

/// Represents the AppleMIDI session state.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Idle,
    Inviting,
    AwaitingOK,
    ClockSync, // Starting clock sync
    Established, // Handshake complete, ready for data
    Terminated,
}

/// Represents a full RTP-MIDI session, managing sending, receiving, journaling, and AppleMIDI handshake.
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

    // --- AppleMIDI State ---
    pub session_state: Arc<Mutex<SessionState>>,
    pub handshake_timer: Arc<Mutex<Option<Pin<Box<Sleep>>>>>,
    pub sync_timer: Arc<Mutex<Option<Pin<Box<Sleep>>>>>,
    pub last_sync_count: Arc<Mutex<u8>>,
    pub initiator_token: u32,
    pub peer_ssrc: Arc<Mutex<Option<u32>>>,
}

impl RtpMidiSession {
    /// Creates a new RTP-MIDI session with handshake state.
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
            session_state: Arc::new(Mutex::new(SessionState::Idle)),
            handshake_timer: Arc::new(Mutex::new(None)),
            sync_timer: Arc::new(Mutex::new(None)),
            last_sync_count: Arc::new(Mutex::new(0)),
            initiator_token: rand::random(),
            peer_ssrc: Arc::new(Mutex::new(None)),
        })
    }

    /// Handles an incoming raw UDP packet, parses it, and processes MIDI commands or journal data.
    pub async fn handle_incoming_packet(&mut self, data: Vec<u8>, event_sender: &tokio::sync::broadcast::Sender<Event>, peer_addr: SocketAddr) {
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

                // --- Gap Detection ---
                let expected_seq = history.iter().next_back().map_or(packet.sequence_number, |&last| last.wrapping_add(1));
                if packet.sequence_number != expected_seq && !history.is_empty() {
                    let mut missing = Vec::new();
                    let mut seq = expected_seq;
                    while seq != packet.sequence_number {
                        if !history.contains(&seq) {
                            missing.push(seq);
                        }
                        seq = seq.wrapping_add(1);
                    }
                    if !missing.is_empty() {
                        warn!("Detected missing RTP-MIDI packets: {:?}. Attempting recovery via journal.", missing);
                        // Try to recover using the journal if present
                        if let Some(journal) = &packet.journal_data {
                            let recovered = self.process_journal(journal, &mut history);
                            let unrecovered: Vec<_> = missing.iter().filter(|seq| !recovered.contains(seq)).cloned().collect();
                            // --- Enhanced: Reconstruct and emit repaired MIDI commands for each missing sequence ---
                            for seq in &missing {
                                if let Some(entry) = journal.entries().iter().find(|e| &e.sequence_nr == seq) {
                                    // Convert TimedMidiCommand to raw MIDI bytes for event
                                    let mut repaired_bytes = Vec::new();
                                    for cmd in &entry.commands {
                                        // Serialize each TimedMidiCommand to raw MIDI bytes
                                        let mut midi_bytes = Vec::new();
                                        if let Ok(()) = rtp_midi_core::journal_engine::serialize_midi_command(&cmd.command, &mut midi_bytes) {
                                            repaired_bytes.extend(midi_bytes);
                                        }
                                    }
                                    if !repaired_bytes.is_empty() {
                                        let _ = event_sender.send(Event::JournalBasedRepair {
                                            repaired_commands: repaired_bytes,
                                            peer: peer_addr,
                                        });
                                    }
                                }
                            }
                            if !unrecovered.is_empty() {
                                warn!("Could not recover all missing packets from journal: {:?}", unrecovered);
                            } else {
                                info!("Successfully recovered all missing packets from journal.");
                            }
                        } else {
                            warn!("No journal present to recover missing packets: {:?}", missing);
                        }
                    }
                }

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
        if *self.session_state.lock().await != SessionState::Established {
            return Err(anyhow!("Cannot send MIDI data: session not established."));
        }

        let peer_guard = self.peer_addr.lock().await;
        let peer = peer_guard.ok_or_else(|| anyhow!("Cannot send MIDI, no peer connected."))?;

        let mut seq_num_lock = self.sequence_number.lock().await;
        let mut last_sent_ts_lock = self.last_sent_timestamp.lock().await;
        
        let current_timestamp = (tokio::time::Instant::now().elapsed().as_secs_f64() * MIDI_CLOCK_HZ) as u32;
        let delta_time_is_zero = commands.iter().all(|cmd| cmd.delta_time == 0);
        let is_sysex_start = commands.first().is_some_and(|cmd| cmd.command.first().is_some_and(|&b| b == 0xF0));

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
            commands: commands.iter().filter_map(|msg| {
                match rtp_midi_core::parse_midi_message(&msg.command) {
                    Ok((cmd, _)) => Some(TimedMidiCommand {
                        delta_time: msg.delta_time,
                        command: cmd,
                    }),
                    Err(e) => {
                        warn!("Failed to parse MidiMessage for journaling: {}", e);
                        None
                    }
                }
            }).collect(),
        });

        // Increment sequence number for the next packet.
        *seq_num_lock = seq_num_lock.wrapping_add(1);

        Ok(())
    }

    /// Processes a journal from an incoming packet, identifies missing packets,
    /// and updates the receive history. Returns a set of sequence numbers recovered.
    fn process_journal(&self, journal_data: &JournalData, history: &mut BTreeSet<u16>) -> std::collections::HashSet<u16> {
        let entries = match journal_data {
            JournalData::Enhanced { entries, .. } => entries,
        };

        let mut recovered = std::collections::HashSet::new();
        if entries.is_empty() {
            return recovered;
        }

        for entry in entries {
            if history.insert(entry.sequence_nr) {
                recovered.insert(entry.sequence_nr);
            }
        }
        recovered
    }

    /// Initiates the AppleMIDI handshake by sending an IN (invitation) message with retries and timeout.
    pub async fn initiate_handshake(&self, peer_addr: SocketAddr, name: &str) -> Result<()> {
        let mut state = self.session_state.lock().await;
        if *state != SessionState::Idle {
            return Err(anyhow!("Handshake already in progress or established."));
        }
        *state = SessionState::AwaitingOK;
        *self.peer_addr.lock().await = Some(peer_addr);

        info!("Initiating handshake with {} at {}", name, peer_addr);

        let invitation = AppleMidiMessage::Invitation(Invitation::new(
            self.initiator_token,
            self.ssrc,
            self.name.clone(),
        ));

        let mut attempts = 0;
        let max_attempts = 3;
        let timeout = Duration::from_secs(2);
        while attempts < max_attempts {
            self.send_control_message(&invitation).await?;
            info!("Sent IN (invitation), attempt {}", attempts + 1);
            // Wait for OK or timeout
            let mut ok_received = false;
            for _ in 0..20 {
                sleep(timeout / 20).await;
                let state_now = self.session_state.lock().await.clone();
                if state_now == SessionState::Established {
                    ok_received = true;
                    break;
                } else if state_now == SessionState::Terminated {
                    warn!("Session terminated during handshake");
                    return Err(anyhow!("Session terminated during handshake"));
                }
            }
            if ok_received {
                info!("Handshake OK received, session established.");
                // Emit event: SessionEstablished (add your event bus logic here)
                break;
            }
            attempts += 1;
        }
        if *self.session_state.lock().await != SessionState::Established {
            warn!("Handshake failed after {} attempts.", max_attempts);
            *self.session_state.lock().await = SessionState::Idle;
            // Emit event: SessionRejected (add your event bus logic here)
            return Err(anyhow!("Handshake failed: no OK received"));
        }
        Ok(())
    }

    /// Handles a control message from a remote peer.
    pub async fn handle_control_message(&self, msg: AppleMidiMessage, peer_addr: SocketAddr) -> Result<()> {
        let mut state = self.session_state.lock().await;
        match msg {
            AppleMidiMessage::Invitation(inv) => {
                if *state == SessionState::Idle {
                    info!("Received invitation from {}. Accepting.", inv.name);
                    *self.peer_addr.lock().await = Some(peer_addr);
                    *self.peer_ssrc.lock().await = Some(inv.header.ssrc);
                    // Send OK response
                    let response = AppleMidiMessage::InvitationAccepted(InvitationAccepted::new(
                        inv.header.initiator_token,
                        self.ssrc,
                        self.name.clone(),
                    ));
                    self.send_control_message(&response).await?;
                    *state = SessionState::Established; // We are now established
                }
            }
            AppleMidiMessage::InvitationAccepted(ok) => {
                if *state == SessionState::AwaitingOK && ok.header.initiator_token == self.initiator_token {
                    info!("Invitation accepted by {}. Session established.", ok.name);
                    *self.peer_ssrc.lock().await = Some(ok.header.ssrc);
                    *state = SessionState::Established;
                    // Optional: start clock sync
                    self.initiate_clock_sync(peer_addr).await?;
                }
            }
            AppleMidiMessage::Sync(sync) => {
                // This is a simplified clock sync. A full implementation would handle CK0, CK1, CK2.
                if *state == SessionState::Established || *state == SessionState::ClockSync {
                    if sync.count == 0 { // Peer initiated sync (CK0)
                        info!("Received CK0, responding with CK1.");
                        let response = AppleMidiMessage::Sync(AppleMidiSync::new(
                            self.ssrc,
                            1,
                            sync.timestamps, // Echo back timestamps
                        ));
                        self.send_control_message(&response).await?;
                    } else if sync.count == 1 { // We initiated, this is CK1 response
                        info!("Received CK1, sending CK2 and finalizing sync.");
                        let response = AppleMidiMessage::Sync(AppleMidiSync::new(
                            self.ssrc,
                            2,
                            sync.timestamps,
                        ));
                        self.send_control_message(&response).await?;
                        *state = SessionState::ClockSync;
                    } else if sync.count == 2 {
                        info!("Received CK2, clock sync complete.");
                        *state = SessionState::Established;
                    }
                }
            }
            _ => warn!("Received unexpected control message in state {:?}: {:?}", *state, msg),
        }
        Ok(())
    }

    /// Sends a control message to the currently connected peer.
    async fn send_control_message(&self, msg: &AppleMidiMessage) -> Result<()> {
        if let Some(peer) = *self.peer_addr.lock().await {
            let data = msg.serialize();
            if let Some(handler) = &*self.outgoing_packet_handler.lock().await {
                let handler_clone = handler.clone();
                handler_clone.lock().await(peer.ip().to_string(), peer.port(), data.to_vec());
            }
            Ok(())
        } else {
            Err(anyhow!("Cannot send control message: no peer connected."))
        }
    }

    /// Initiates the CK (clock sync) exchange with retries and timeout.
    pub async fn initiate_clock_sync(&self, peer_addr: SocketAddr) -> Result<()> {
        let mut state = self.session_state.lock().await;
        if *state != SessionState::Established {
            warn!("Cannot initiate clock sync: session not established.");
            return Ok(());
        }
        *state = SessionState::ClockSync;
        info!("Initiating clock sync with peer at {}", peer_addr);

        let sync_msg = AppleMidiMessage::Sync(AppleMidiSync::new(
            self.ssrc,
            0, // This is CK0
            [0, 0, 0], // Timestamps are filled by the receiver
        ));

        let mut attempts = 0;
        let max_attempts = 3;
        let timeout = Duration::from_secs(2);
        while attempts < max_attempts {
            self.send_control_message(&sync_msg).await?;
            info!("Sent CK0 (clock sync), attempt {}", attempts + 1);
            // Wait for CK1 or timeout
            let mut ck1_received = false;
            for _ in 0..20 {
                sleep(timeout / 20).await;
                let state_now = self.session_state.lock().await.clone();
                if state_now == SessionState::Established {
                    ck1_received = true;
                    break;
                } else if state_now == SessionState::Terminated {
                    warn!("Session terminated during clock sync");
                    return Err(anyhow!("Session terminated during clock sync"));
                }
            }
            if ck1_received {
                info!("Clock sync complete.");
                // Emit event: SyncStatusChanged (add your event bus logic here)
                break;
            }
            attempts += 1;
        }
        if *self.session_state.lock().await != SessionState::Established {
            warn!("Clock sync failed after {} attempts.", max_attempts);
            *self.session_state.lock().await = SessionState::Idle;
            // Emit event: SyncFailed (add your event bus logic here)
            return Err(anyhow!("Clock sync failed: no CK1/CK2 received"));
        }
        Ok(())
    }

    /// Gracefully terminates the session.
    pub async fn end_session(&self) -> Result<()> {
        let mut state = self.session_state.lock().await;
        *state = SessionState::Terminated;
        // Clean up resources, etc.
        Ok(())
    }

    // TODO: Add more methods for handshake/CK retries, timeouts, and cleanup
    // TODO: Add event emission for all state changes and sync status
    // TODO: Add tests for handshake and CK edge cases
}

#[async_trait::async_trait]
impl DataStreamNetSender for RtpMidiSession {
    fn init(&mut self) -> Result<(), StreamError> {
        // Zde lze inicializovat síťové zdroje, pokud je potřeba
        Ok(())
    }
    fn send(&mut self, _ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        // Odeslání raw MIDI packetu (payload) s timestampem ts
        // Zde by bylo potřeba převést payload na MidiMessage a zavolat send_midi
        // Pro jednoduchost předpokládáme, že payload je již správně připravený
        // (v praxi by zde byla serializace/deserializace)
        // TODO: Implementace podle konkrétního formátu
        match rtp_midi_core::parse_midi_message(payload) {
            Ok((_cmd, _)) => {
                // Wrap in MidiMessage with delta_time = 0 for now
                let midi_msg = MidiMessage { delta_time: 0, command: payload.to_vec() };
                // This is a sync function, but send_midi is async, so we just log for now
                // In production, this should be refactored for async context
                warn!("DataStreamNetSender::send called, but send_midi is async. Message: {:?}", midi_msg);
                Ok(())
            },
            Err(e) => {
                warn!("Failed to parse MIDI payload in DataStreamNetSender::send: {}", e);
                Err(StreamError::Other(format!("Failed to parse MIDI payload: {}", e)))
            }
        }
    }
}

#[async_trait::async_trait]
impl DataStreamNetReceiver for RtpMidiSession {
    fn init(&mut self) -> Result<(), StreamError> {
        Ok(())
    }
    fn poll(&mut self, _buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError> {
        // Zde by se zpracovával příchozí packet a naplnil buf
        // Prozatím pouze stub
        Ok(None)
    }
}

// Helper to get entries from JournalData
trait JournalEntries {
    fn entries(&self) -> &Vec<JournalEntry>;
}
impl JournalEntries for JournalData {
    fn entries(&self) -> &Vec<JournalEntry> {
        match self {
            JournalData::Enhanced { entries, .. } => entries,
        }
    }
}
