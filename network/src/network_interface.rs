// rtp_midi_lib/src/network_interface.rs

use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, watch};

use rtp_midi_core::event_bus::Event;

pub async fn start_network_interface(
    mut receiver: broadcast::Receiver<Event>,
    sender: broadcast::Sender<Event>,
    listen_port: u16,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", listen_port)).await?);
    info!("Network interface bound to port {}", listen_port);

    let r_socket = socket.clone();
    let r_sender = sender.clone();
    let mut shutdown_recv = shutdown.clone();
    let udp_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        loop {
            tokio::select! {
                _ = shutdown_recv.changed() => {
                    if *shutdown_recv.borrow() {
                        info!("Network interface UDP receive loop shutting down.");
                        break;
                    }
                }
                res = r_socket.recv_from(&mut buf) => {
                    match res {
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
            }
        }
    });

    while !*shutdown.borrow() {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("Network interface event loop shutting down.");
                    break;
                }
            }
            res = receiver.recv() => {
                match res {
                    Ok(event) => {
                        if let Event::SendPacket { payload, dest_addr } = event {
                            match socket.send_to(&payload, dest_addr).await {
                                Ok(len) => info!("Sent {} bytes to {}", len, dest_addr),
                                Err(e) => error!("Failed to send UDP packet to {}: {}", dest_addr, e),
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    let _ = udp_task.await;
    Ok(())
}
