use crate::event_bus::{Event};
use tokio::sync::mpsc::{Sender, Receiver};
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Disconnected,
    Handshaking,
    Connected,
    Syncing,
}

pub struct SessionManager {
    event_sender: Sender<Event>,
    event_receiver: Receiver<Event>,
    state: SessionState,
    peer_addr: Option<SocketAddr>,
    initiator_token: Option<u32>,
    ssrc: Option<u32>,
    // Add more fields as needed for clock sync, etc.
}

impl SessionManager {
    pub fn new(event_sender: Sender<Event>, event_receiver: Receiver<Event>) -> Self {
        Self {
            event_sender,
            event_receiver,
            state: SessionState::Disconnected,
            peer_addr: None,
            initiator_token: None,
            ssrc: None,
        }
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.event_receiver.recv().await {
            self.process_event(event).await;
        }
    }

    async fn process_event(&mut self, event: Event) {
        match event {
            Event::RawPacketReceived { payload, source_addr } => {
                // Parse AppleMIDI control packet (IN, OK, NO, BY, CK)
                if let Some(cmd) = Self::parse_applemidi_command(&payload) {
                    match cmd {
                        AppleMidiCommand::Invitation { initiator_token, ssrc, name } => {
                            // Handle invitation (IN)
                            self.handle_invitation(initiator_token, ssrc, name, source_addr).await;
                        }
                        AppleMidiCommand::InvitationAccepted { initiator_token, ssrc, name } => {
                            // Handle invitation accepted (OK)
                            self.handle_invitation_accepted(initiator_token, ssrc, name, source_addr).await;
                        }
                        AppleMidiCommand::InvitationRejected { initiator_token, ssrc } => {
                            // Handle invitation rejected (NO)
                            self.handle_invitation_rejected(initiator_token, ssrc, source_addr).await;
                        }
                        AppleMidiCommand::EndSession { ssrc } => {
                            // Handle session termination (BY)
                            self.handle_end_session(ssrc, source_addr).await;
                        }
                        AppleMidiCommand::ClockSync { count, timestamps, ssrc } => {
                            // Handle clock sync (CK)
                            self.handle_clock_sync(count, timestamps, ssrc, source_addr).await;
                        }
                    }
                } else {
                    // Not a control packet; ignore or handle as MIDI data
                }
            }
            _ => {}
        }
    }

    // Placeholder: parse AppleMIDI control command from payload
    fn parse_applemidi_command(payload: &[u8]) -> Option<AppleMidiCommand> {
        if payload.len() < 2 {
            return None;
        }
        let cmd = &payload[0..2];
        match cmd {
            b"IN" | b"OK" | b"NO" => {
                // Invitation, Accepted, or Rejected
                if payload.len() < 16 {
                    return None;
                }
                let protocol_version = u16::from_be_bytes([payload[2], payload[3]]);
                let initiator_token = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let ssrc = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
                // Name is optional and null-terminated
                let name = if payload.len() > 12 {
                    let name_bytes = &payload[12..];
                    let nul_pos = name_bytes.iter().position(|&b| b == 0);
                    if let Some(pos) = nul_pos {
                        String::from_utf8(name_bytes[..pos].to_vec()).ok()
                    } else {
                        String::from_utf8(name_bytes.to_vec()).ok()
                    }
                } else {
                    None
                };
                match cmd {
                    b"IN" => Some(AppleMidiCommand::Invitation { initiator_token, ssrc, name }),
                    b"OK" => Some(AppleMidiCommand::InvitationAccepted { initiator_token, ssrc, name }),
                    b"NO" => Some(AppleMidiCommand::InvitationRejected { initiator_token, ssrc }),
                    _ => None,
                }
            }
            b"BY" => {
                // End Session
                if payload.len() < 8 {
                    return None;
                }
                let ssrc = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                Some(AppleMidiCommand::EndSession { ssrc })
            }
            b"CK" => {
                // Clock Sync
                if payload.len() < 28 {
                    return None;
                }
                let count = payload[2];
                let ssrc = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let mut timestamps = [0u64; 3];
                for i in 0..3 {
                    let start = 8 + i * 8;
                    let end = start + 8;
                    if end > payload.len() { return None; }
                    timestamps[i] = u64::from_be_bytes(payload[start..end].try_into().ok()?);
                }
                Some(AppleMidiCommand::ClockSync { count, timestamps, ssrc })
            }
            _ => None,
        }
    }

    // Placeholder: handle invitation (IN)
    async fn handle_invitation(&mut self, initiator_token: u32, ssrc: u32, name: Option<String>, source_addr: SocketAddr) {
        // AppleMIDI handshake responder logic
        match self.state {
            SessionState::Disconnected => {
                // Accept invitation, store peer info
                self.peer_addr = Some(source_addr);
                self.initiator_token = Some(initiator_token);
                self.ssrc = Some(ssrc);
                self.state = SessionState::Handshaking;
                // Build OK response packet
                let mut payload = Vec::new();
                payload.extend_from_slice(b"OK");
                payload.extend_from_slice(&2u16.to_be_bytes()); // Protocol version 2
                payload.extend_from_slice(&initiator_token.to_be_bytes());
                // Our SSRC (for now, just echo peer's SSRC; in real impl, generate unique)
                payload.extend_from_slice(&ssrc.to_be_bytes());
                // Name (optional, null-terminated)
                if let Some(ref n) = name {
                    payload.extend_from_slice(n.as_bytes());
                    payload.push(0);
                }
                // Send OK response
                let _ = self.event_sender.send(Event::SendPacket {
                    payload,
                    dest_addr: source_addr,
                }).await;
                // Optionally: log or notify
            }
            _ => {
                // Already in session or handshaking; reject new invitation
                let mut payload = Vec::new();
                payload.extend_from_slice(b"NO");
                payload.extend_from_slice(&2u16.to_be_bytes());
                payload.extend_from_slice(&initiator_token.to_be_bytes());
                payload.extend_from_slice(&ssrc.to_be_bytes());
                // No name for NO response
                let _ = self.event_sender.send(Event::SendPacket {
                    payload,
                    dest_addr: source_addr,
                }).await;
            }
        }
    }

    // Placeholder: handle invitation accepted (OK)
    async fn handle_invitation_accepted(&mut self, _initiator_token: u32, _ssrc: u32, _name: Option<String>, _source_addr: SocketAddr) {
        // TODO: Update state, proceed to next handshake step
    }

    // Placeholder: handle invitation rejected (NO)
    async fn handle_invitation_rejected(&mut self, _initiator_token: u32, _ssrc: u32, _source_addr: SocketAddr) {
        // TODO: Handle rejection, reset state
    }

    // Placeholder: handle session termination (BY)
    async fn handle_end_session(&mut self, _ssrc: u32, _source_addr: SocketAddr) {
        // TODO: Clean up session, reset state
    }

    // Placeholder: handle clock sync (CK)
    async fn handle_clock_sync(&mut self, _count: u8, _timestamps: [u64; 3], _ssrc: u32, _source_addr: SocketAddr) {
        // TODO: Implement clock sync exchange, update state
        // On successful sync, publish SessionEstablished
    }
}

// Enum for AppleMIDI control commands
#[derive(Debug, Clone)]
enum AppleMidiCommand {
    Invitation { initiator_token: u32, ssrc: u32, name: Option<String> },
    InvitationAccepted { initiator_token: u32, ssrc: u32, name: Option<String> },
    InvitationRejected { initiator_token: u32, ssrc: u32 },
    EndSession { ssrc: u32 },
    ClockSync { count: u8, timestamps: [u64; 3], ssrc: u32 },
} 