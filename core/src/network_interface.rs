use crate::event_bus::Event;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;

pub struct NetworkInterface {
    socket: UdpSocket,
    event_sender: Sender<Event>,
}

impl NetworkInterface {
    pub async fn new(bind_addr: &str, event_sender: Sender<Event>) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        Ok(Self {
            socket,
            event_sender,
        })
    }

    pub async fn start_listening(&self) {
        let mut buf = [0u8; 2048];
        while let Ok((len, src)) = self.socket.recv_from(&mut buf).await {
            let payload = buf[..len].to_vec();
            let event = Event::RawPacketReceived {
                payload,
                source_addr: src,
            };
            let _ = self.event_sender.send(event).await;
        }
    }

    pub async fn handle_send_packet(&self, payload: Vec<u8>, dest_addr: SocketAddr) {
        let _ = self.socket.send_to(&payload, dest_addr).await;
    }
}
