// rtp_midi_lib/src/network_interface.rs

use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use log::{error, info};
use std::sync::Arc;

use rtp_midi_core::event_bus::Event;

pub async fn start_network_interface(
    mut receiver: broadcast::Receiver<Event>,
    sender: broadcast::Sender<Event>,
    listen_port: u16,
) -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", listen_port)).await?);
    info!("Network interface bound to port {}", listen_port);

    let r_socket = socket.clone();
    let r_sender = sender.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        loop {
            match r_socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    if let Err(e) = r_sender.send(Event::RawPacketReceived {
                        payload: buf[..len].to_vec(),
                        source_addr: addr,
                    }) {
                        error!("Failed to send RawPacketReceived event: {}", e);
                    }
                },
                Err(e) => error!("UDP receive error: {}", e),
            }
        }
    });

    while let Ok(event) = receiver.recv().await {
        if let Event::SendPacket { payload, dest_addr } = event {
            match socket.send_to(&payload, dest_addr).await {
                Ok(len) => info!("Sent {} bytes to {}", len, dest_addr),
                Err(e) => error!("Failed to send UDP packet to {}: {}", dest_addr, e),
            }
        }
    }

    Ok(())
} 