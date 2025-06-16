use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use log::{info, warn, error};

#[derive(Serialize, Deserialize, Debug, Clone)]
enum PeerType {
    AudioServer,
    ClientApp,
}

#[derive(Serialize, Deserialize, Debug)]
struct RegisterPayload {
    peer_type: PeerType,
    client_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SignalingMessage {
    message_type: String,
    sender_id: String,
    receiver_id: Option<String>,
    payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct ClientNotification {
    client_id: String,
    peer_type: PeerType,
}

#[derive(Clone)]
struct Clients {
    peers: Arc<Mutex<HashMap<String, (PeerType, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)>>>,
}

impl Clients {
    fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn register(&self, client_id: String, peer_type: PeerType, tx: mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
        let mut peers = self.peers.lock().unwrap();
        peers.insert(client_id, (peer_type, tx));
    }

    fn unregister(&self, client_id: &str) {
        let mut peers = self.peers.lock().unwrap();
        peers.remove(client_id);
    }

    fn get_audio_server(&self) -> Option<(String, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)> {
        let peers = self.peers.lock().unwrap();
        for (id, (peer_type, tx)) in peers.iter() {
            if let PeerType::AudioServer = peer_type {
                return Some((id.clone(), tx.clone()));
            }
        }
        None
    }

    fn get_peer(&self, client_id: &str) -> Option<mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>> {
        let peers = self.peers.lock().unwrap();
        peers.get(client_id).map(|(_, tx)| tx.clone())
    }

    fn get_all_clients(&self) -> Vec<(String, PeerType)> {
        let peers = self.peers.lock().unwrap();
        peers.iter().map(|(id, (peer_type, _))| (id.clone(), peer_type.clone())).collect()
    }
}

async fn handle_connection(clients: Clients, stream: TcpStream) {
    info!("New connection from {}", stream.peer_addr().unwrap());
    
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Error during WebSocket handshake: {}", e);
            return;
        }
    };
    
    let (mut ws_write, mut ws_read) = ws_stream.split();
    
    let (tx, mut rx) = mpsc::channel::<Result<Message, tokio_tungstenite::tungstenite::Error>>(64);
    
    let mut client_id = String::new();
    let mut peer_type = None;
    
    // Listen for incoming messages
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_write.send(msg.unwrap()).await {
                error!("Error sending message over WebSocket: {}", e);
                break;
            }
        }
    });
    
    while let Some(msg) = ws_read.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                error!("Error reading message from WebSocket: {}", e);
                break;
            }
        };
        
        if msg.is_close() {
            info!("WebSocket connection closed by peer");
            break;
        }
        
        if let Message::Text(text) = msg {
            match serde_json::from_str::<SignalingMessage>(&text) {
                Ok(signaling_msg) => {
                    match signaling_msg.message_type.as_str() {
                        "register" => {
                            if let Ok(register_payload) = serde_json::from_value::<RegisterPayload>(signaling_msg.payload) {
                                client_id = register_payload.client_id.clone();
                                peer_type = Some(register_payload.peer_type.clone());
                                
                                clients.register(client_id.clone(), register_payload.peer_type, tx.clone());
                                info!("Registered client: {} (type: {:?})", client_id, peer_type);
                                
                                // Inform client of successful registration
                                let response = SignalingMessage {
                                    message_type: "register_success".to_string(),
                                    sender_id: "server".to_string(),
                                    receiver_id: Some(client_id.clone()),
                                    payload: serde_json::json!({
                                        "message": "Successfully registered",
                                        "registered_id": client_id,
                                        "clients": clients.get_all_clients()
                                    }),
                                };
                                
                                if let Ok(response_str) = serde_json::to_string(&response) {
                                    let _ = tx.send(Ok(Message::text(response_str))).await;
                                } else {
                                    error!("Failed to serialize register_success response");
                                }
                                
                                // Inform audio server about the new client
                                if let Some(PeerType::ClientApp) = peer_type {
                                    notify_audio_server_of_new_client(&clients, &client_id).await;
                                }
                            } else {
                                warn!("Failed to parse register payload from client {}", client_id);
                            }
                        },
                        "list_peers" => {
                            let current_clients = clients.get_all_clients();
                            let response = SignalingMessage {
                                message_type: "peer_list".to_string(),
                                sender_id: "server".to_string(),
                                receiver_id: Some(client_id.clone()), // Send back to the requester
                                payload: serde_json::json!({
                                    "peers": current_clients
                                }),
                            };
                             
                            if let Ok(response_str) = serde_json::to_string(&response) {
                                if tx.send(Ok(Message::text(response_str))).await.is_err() {
                                    error!("Failed to send peer_list to client {}", client_id);
                                }
                            } else {
                                error!("Failed to serialize peer_list response for client {}", client_id);
                            }
                        },
                        "offer" | "answer" | "ice_candidate" => {
                            if let Some(receiver_id) = &signaling_msg.receiver_id {
                                if let Some(peer_tx) = clients.get_peer(receiver_id) {
                                    if let Ok(msg_str) = serde_json::to_string(&signaling_msg) {
                                        if let Err(e) = peer_tx.send(Ok(Message::text(msg_str))).await {
                                            error!("Error forwarding message to {}: {}", receiver_id, e);
                                        }
                                    } else {
                                        error!("Failed to serialize signaling message for forwarding to {}", receiver_id);
                                    }
                                } else {
                                    warn!("Target peer {} not found for message type {}", receiver_id, signaling_msg.message_type);
                                }
                            } else {
                                warn!("Receiver ID missing for message type {}", signaling_msg.message_type);
                            }
                        },
                        _ => {
                            warn!("Unknown message type received: {}", signaling_msg.message_type);
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to parse signaling message: {}", e);
                }
            }
        } else if msg.is_binary() {
            warn!("Received binary message, expected text");
        }
    }
    
    if !client_id.is_empty() {
        info!("Client {} disconnected", client_id);
        clients.unregister(&client_id);
    }
}

async fn notify_audio_server_of_new_client(clients: &Clients, client_id: &str) {
    if let Some((server_id, server_tx)) = clients.get_audio_server() {
        let notification_payload = SignalingMessage {
            message_type: "new_client".to_string(),
            sender_id: "server".to_string(),
            receiver_id: Some(server_id.clone()),
            payload: serde_json::json!({
                "client_id": client_id
            }),
        };
        
        if let Ok(msg_str) = serde_json::to_string(&notification_payload) {
            if server_tx.send(Ok(Message::text(msg_str))).await.is_err() {
                error!("Failed to notify audio server {} about client {}", server_id, client_id);
            }
        } else {
            error!("Failed to serialize new_client notification for audio server {}", server_id);
        }
    } else {
        info!("No audio server registered to notify about new client {}", client_id);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("Signaling server running on ws://{}/signaling", addr);

    let clients = Clients::new();

    while let Ok((stream, _)) = listener.accept().await {
        let peer_addr = stream.peer_addr()?;
        info!("Accepted connection from: {}", peer_addr);
        
        let clients_clone = clients.clone();
        tokio::spawn(async move {
            handle_connection(clients_clone, stream).await;
        });
    }

    Ok(())
}