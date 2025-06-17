use anyhow::Result;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message, connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;
use uuid::Uuid;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_remote::TrackRemote;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Debug)]
struct ClientConnection {
    peer_connection: Arc<RTCPeerConnection>,
    data_channel: Option<Arc<RTCDataChannel>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let server_id = "audio_server_main".to_string();
    let signaling_url = "ws://localhost:8080/signaling";

    let url = Url::parse(signaling_url)?;
    let (ws_stream, _) = connect_async(url.as_str()).await?;
    println!("[AudioServer] Připojeno k signalizačnímu serveru.");

    let (ws_write, mut ws_read) = ws_stream.split();
    let ws_write = Arc::new(Mutex::new(ws_write));

    let register_payload = RegisterPayload {
        peer_type: PeerType::AudioServer,
        client_id: server_id.clone(),
    };

    let register_msg = SignalingMessage {
        message_type: "register".to_string(),
        sender_id: server_id.clone(),
        receiver_id: None,
        payload: serde_json::to_value(register_payload)?,
    };

    let register_msg_str = serde_json::to_string(&register_msg)?;
    ws_write.lock().unwrap().send(Message::text(register_msg_str)).await?;

    let client_connections = Arc::new(Mutex::new(HashMap::<String, ClientConnection>::new()));

    while let Some(msg) = ws_read.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("[AudioServer] Chyba při čtení zprávy: {}", e);
                break;
            }
        };

        if let Message::Text(text) = msg {
            match serde_json::from_str::<SignalingMessage>(&text) {
                Ok(signaling_msg) => {
                    let message_type = signaling_msg.message_type.clone();
                    let sender_id = signaling_msg.sender_id.clone();

                    match message_type.as_str() {
                        "offer" => {
                            info!("[AudioServer] Přijata nabídka od klienta: {}", sender_id);
                            let ws_write_clone = Arc::clone(&ws_write);
                            let server_id_clone = server_id.clone();
                            let client_connections_clone = Arc::clone(&client_connections);

                            tokio::spawn(async move {
                                if let Err(e) = handle_offer(
                                    &signaling_msg,
                                    &sender_id,
                                    &server_id_clone,
                                    ws_write_clone,
                                    client_connections_clone,
                                )
                                .await
                                {
                                    error!("[AudioServer] Chyba při zpracování nabídky: {}", e);
                                }
                            });
                        }
                        _ => {
                            warn!("[AudioServer] Neznámý typ zprávy: {}", message_type);
                        }
                    }
                }
                Err(e) => {
                    error!("[AudioServer] Chyba při parsování zprávy: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_offer(
    msg: &SignalingMessage,
    client_id: &str,
    server_id: &str,
    ws_write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    client_connections: Arc<Mutex<HashMap<String, ClientConnection>>>,
) -> Result<()> {
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs()?;
    let api = APIBuilder::new().with_media_engine(media_engine).build();
    let mut rtc_config = RTCConfiguration::default();
    rtc_config.ice_servers = vec![RTCIceServer {
        urls: vec!["stun:stun.l.google.com:19302".to_string()],
        ..Default::default()
    }];
    let peer_connection: Arc<RTCPeerConnection> = Arc::new(api.new_peer_connection(rtc_config).await?);
    
    let pc_clone = Arc::clone(&peer_connection);
    let ws_write_clone = Arc::clone(&ws_write);
    let server_id_clone = server_id.to_string();
    let client_id_clone = client_id.to_string();
    
    peer_connection.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidateInit>| {
        let ws_write = ws_write_clone.clone();
        let server_id = server_id_clone.clone();
        let client_id = client_id_clone.clone();
    
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_msg = SignalingMessage {
                    message_type: "ice_candidate".to_string(),
                    sender_id: server_id,
                    receiver_id: Some(client_id),
                    payload: json!({ "candidate": candidate }),
                };
    
                if let Ok(msg_str) = serde_json::to_string(&candidate_msg) {
                    if let Err(e) = ws_write.lock().unwrap().send(Message::text(msg_str)).await {
                        error!("[AudioServer] Chyba při odesílání ICE kandidáta: {}", e);
                    }
                }
            }
        })
    })).await;
    
    let client_id_for_dc = client_id.to_string();
    let client_connections_clone = Arc::clone(&client_connections);
    
    peer_connection.on_data_channel(Box::new(move |data_channel: Arc<RTCDataChannel>| {
        let client_id = client_id_for_dc.clone();
        let connections = client_connections_clone.clone();
    
        Box::pin(async move {
            info!("[AudioServer] Přijat data channel od klienta: {}", client_id);
            let dc_clone = Arc::clone(&data_channel);
            let client_id_clone = client_id.clone();
    
            data_channel.on_message(Box::new(move |msg| {
                Box::pin(async move {
                    if msg.is_binary() {
                        info!("[AudioServer] MIDI data od klienta {}: {:?}", client_id_clone, msg.data);
                    }
                })
            }));
    
            let mut connections_lock = connections.lock().unwrap();
            if let Some(conn) = connections_lock.get_mut(&client_id) {
                conn.data_channel = Some(Arc::clone(&data_channel));
            }
        })
    }));
    
    let client_id_on_track = client_id.to_string();
    peer_connection.on_track(Box::new(move |track: Option<Arc<TrackRemote>>, _receiver| {
        let track_client_id = client_id_on_track.clone();
        Box::pin(async move {
            if let Some(track) = track {
                info!(
                    "[AudioServer] Přijat stream SSRC {} od klienta {}, typ: {}",
                    track.ssrc(),
                    track_client_id,
                    track.kind()
                );
            }
        })
    }));
    
    if let Some(sdp) = msg.payload.get("sdp").and_then(|v| v.as_str()) {
        let offer = RTCSessionDescription::offer(sdp.to_string())?;
        peer_connection.set_remote_description(offer).await?;
        let answer = peer_connection.create_answer(None).await?;
        peer_connection.set_local_description(answer.clone()).await?;
    
        let answer_msg = SignalingMessage {
            message_type: "answer".to_string(),
            sender_id: server_id.to_string(),
            receiver_id: Some(client_id.to_string()),
            payload: json!({ "sdp": peer_connection.local_description().await.unwrap().sdp }),
        };
    
        if let Ok(msg_str) = serde_json::to_string(&answer_msg) {
            ws_write.lock().unwrap().send(Message::text(msg_str)).await?;
        }
    
        let mut client_conns = client_connections.lock().unwrap();
        client_conns.insert(
            client_id.to_string(),
            ClientConnection {
                peer_connection: Arc::clone(&peer_connection),
                data_channel: None,
            },
        );
        info!("[AudioServer] Odpověď odeslána klientovi {}", client_id);
    }
    
    Ok(())
}