use rosc::{OscPacket, OscMessage, OscType};
use std::net::UdpSocket;
use rtp_midi_core::{DataStreamNetSender, StreamError};
use log::{error, info};

pub struct OscSender {
    socket: UdpSocket,
    target_addr: String,
}

impl OscSender {
    pub fn new(target_addr: &str) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            target_addr: target_addr.to_string(),
        })
    }

    pub fn send_note_on(&self, note: i32, velocity: i32) {
        let msg = OscMessage {
            addr: "/noteOn".to_string(),
            args: vec![OscType::Int(note), OscType::Int(velocity)],
        };
        self.send(msg);
    }

    pub fn send_note_off(&self, note: i32) {
        let msg = OscMessage {
            addr: "/noteOff".to_string(),
            args: vec![OscType::Int(note)],
        };
        self.send(msg);
    }

    pub fn send_cc(&self, controller: i32, value: i32) {
        let msg = OscMessage {
            addr: "/cc".to_string(),
            args: vec![OscType::Int(controller), OscType::Int(value)],
        };
        self.send(msg);
    }

    pub fn send_pitch_bend(&self, bend_value: f32) {
        let msg = OscMessage {
            addr: "/pitchBend".to_string(),
            args: vec![OscType::Float(bend_value)],
        };
        self.send(msg);
    }

    pub fn send_program_change(&self, effect_id: i32) {
        let msg = OscMessage {
            addr: "/config/setEffect".to_string(),
            args: vec![OscType::Int(effect_id)],
        };
        self.send(msg);
    }

    fn send(&self, msg: OscMessage) {
        let packet = OscPacket::Message(msg);
        match rosc::encoder::encode(&packet) {
            Ok(buf) => {
                if let Err(e) = self.socket.send_to(&buf, &self.target_addr) {
                    log::error!("OSC send error: {}", e);
                }
            }
            Err(e) => {
                log::error!("OSC encode error: {}", e);
            }
        }
    }
}

impl DataStreamNetSender for OscSender {
    fn init(&mut self) -> Result<(), StreamError> {
        info!("OSC Sender initialized for target: {}", self.target_addr);
        Ok(())
    }

    fn send(&mut self, _ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        // For OSC, we expect the payload to be a pre-formatted OSC message
        if let Err(e) = self.socket.send_to(payload, &self.target_addr) {
            error!("OSC send error: {}", e);
            return Err(StreamError::NetworkError(e.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::decoder;

    #[test]
    fn test_note_on_encoding() {
        let msg = OscMessage {
            addr: "/noteOn".to_string(),
            args: vec![OscType::Int(60), OscType::Int(100)],
        };
        let packet = OscPacket::Message(msg);
        let buf = rosc::encoder::encode(&packet).unwrap();
        let (decoded, _) = decoder::decode_udp(&buf).unwrap();
        match decoded {
            OscPacket::Message(m) => {
                assert_eq!(m.addr, "/noteOn");
                assert_eq!(m.args, vec![OscType::Int(60), OscType::Int(100)]);
            }
            _ => panic!("Decoded packet is not a message"),
        }
    }
} 