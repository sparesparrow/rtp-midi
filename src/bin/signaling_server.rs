// src/bin/signaling_server.rs

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PeerType {
    AudioServer,
    ClientApp,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegisterPayload {
    pub peer_type: PeerType,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SignalingMessage {
    Register {
        message_type: String,
        sender_id: String,
        payload: RegisterPayload,
    },
    ClientList {
        message_type: String,
        clients: Vec<ClientNotification>,
    },
    Generic {
        message_type: String,
        sender_id: String,
        receiver_id: Option<String>,
        payload: serde_json::Value,
    },
    RegisterSuccess {
        message_type: String,
        payload: serde_json::Value,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientNotification {
    pub client_id: String,
    pub peer_type: PeerType,
}

#[derive(Clone)]
pub struct Clients {
    peers: Arc<Mutex<HashMap<String, (PeerType, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)>>>,
}

impl Clients {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, client_id: String, peer_type: PeerType, tx: mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
        info!("Registering client: {} ({:?})", client_id, peer_type);
        self.peers.lock().unwrap().insert(client_id, (peer_type, tx));
    }

    pub fn unregister(&self, client_id: &str) {
        info!("Unregistering client: {}", client_id);
        self.peers.lock().unwrap().remove(client_id);
    }

    pub fn get_audio_server(&self) -> Option<(String, mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>)> {
        self.peers.lock().unwrap().iter().find_map(|(id, (peer_type, tx))| {
            if *peer_type == PeerType::AudioServer {
                Some((id.clone(), tx.clone()))
            } else {
                None
            }
        })
    }

    pub fn get_peer(&self, client_id: &str) -> Option<mpsc::Sender<Result<Message, tokio_tungstenite::tungstenite::Error>>> {
        self.peers.lock().unwrap().get(client_id).map(|(_, tx)| tx.clone())
    }

    pub fn get_all_clients(&self) -> Vec<(String, PeerType)> {
        self.peers.lock().unwrap().iter().map(|(id, (peer_type, _))| (id.clone(), peer_type.clone())).collect()
    }
}

pub async fn handle_connection(clients: Clients, stream: TcpStream) {
    let peer_addr = match stream.peer_addr() {
        Ok(addr) => addr.to_string(),
        Err(_) => "unknown".to_string(),
    };
    info!("New WebSocket connection from: {}", peer_addr);

    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Error during websocket handshake with {}: {}", peer_addr, e);
            return;
        }
    };
    info!("WebSocket handshake successful with {}", peer_addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel::<Result<Message, tokio_tungstenite::tungstenite::Error>>(100);

    let client_id = Uuid::new_v4().to_string();
    let mut peer_type_opt = None;

    let ws_sender_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                Ok(msg) => {
                    if ws_sender.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    while let Some(msg_res) = ws_receiver.next().await {
        let msg = match msg_res {
            Ok(m) => m,
            Err(e) => {
                error!("Error receiving message from {}: {}", peer_addr, e);
                break;
            }
        };

        if msg.is_text() || msg.is_binary() {
            let text = msg.to_text().unwrap_or("invalid utf8");
            info!("Received message from {}: {}", peer_addr, text);

            match serde_json::from_str::<SignalingMessage>(text) {
                Ok(SignalingMessage::Register { payload, .. }) => {
                    info!("Client {} registered as {:?}", payload.client_id, payload.peer_type);
                    peer_type_opt = Some(payload.peer_type.clone());
                    clients.register(payload.client_id.clone(), payload.peer_type.clone(), tx.clone());

                    let all_clients: Vec<ClientNotification> = clients
                        .get_all_clients()
                        .into_iter()
                        .map(|(id, peer_type)| ClientNotification { client_id: id, peer_type })
                        .collect();
                    let response = SignalingMessage::ClientList {
                        message_type: "client_list".to_string(),
                        clients: all_clients,
                    };

                    if let Ok(msg_text) = serde_json::to_string(&response) {
                        if let Err(e) = tx.send(Ok(Message::Text(msg_text.into()))).await {
                            error!("Failed to send client list to new client: {}", e);
                        }
                    }

                    if payload.peer_type != PeerType::AudioServer {
                        notify_audio_server_of_new_client(&clients, &payload.client_id).await;
                    }
                }
                _ => warn!("Unhandled signaling message type or malformed message: {}", text),
            }
        } else if msg.is_close() {
            info!("Connection closed by client: {}", peer_addr);
            break;
        }
    }

    clients.unregister(&client_id);
    if let Some(peer_type) = peer_type_opt {
        info!("Client {} ({:?}) disconnected.", client_id, peer_type);
    }
    ws_sender_task.abort();
}

async fn notify_audio_server_of_new_client(clients: &Clients, client_id: &str) {
    if let Some((audio_server_id, audio_server_tx)) = clients.get_audio_server() {
        info!("Notifying audio server {} about new client {}", audio_server_id, client_id);
        let notification = ClientNotification {
            client_id: client_id.to_string(),
            peer_type: PeerType::ClientApp,
        };
        let msg = SignalingMessage::ClientList {
            message_type: "client_list".to_string(),
            clients: vec![notification],
        };
        if let Ok(msg_text) = serde_json::to_string(&msg) {
            if let Err(e) = audio_server_tx.send(Ok(Message::Text(msg_text.into()))).await {
                error!("Failed to notify audio server: {}", e);
            }
        }
    }
}

pub async fn run_server(listener: TcpListener) -> anyhow::Result<()> {
    info!("Signaling server listening on {}", listener.local_addr()?);
    let clients = Clients::new();

    loop {
        let (stream, _) = listener.accept().await?;
        let clients_clone = clients.clone();
        tokio::spawn(handle_connection(clients_clone, stream));
    }
}


// Hlavní funkce pro spuštění binárky
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Inicializace loggeru
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    
    info!("Starting signaling server on {}", addr);

    // Volání funkce `run_server`, kterou jsme zkopírovali výše
    if let Err(e) = run_server(listener).await {
        error!("Signaling server failed: {}", e);
    }

    Ok(())
} 