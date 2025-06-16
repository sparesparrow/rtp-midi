use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_remote::TrackRemote;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use opus::{Decoder, Channels};
use url::Url;

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
    receiver_id: Option,
    payload: serde_json::Value,
}

#[derive(Debug)]
struct ClientConnection {
    peer_connection: Arc<RTCPeerConnection>,
    data_channel: Option<Arc<RTCDataChannel>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_id = "audio_server_main".to_string();
    let signaling_url = "ws://localhost:8080/signaling";
    
    let url = Url::parse(signaling_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    println!("[AudioServer] Připojeno k signalizačnímu serveru.");
    
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let ws_write = Arc::new(Mutex::new(ws_write));
    
    // Registrace audio serveru
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
    ws_write.lock().await.send(Message::text(register_msg_str)).await?;
    
    // Spravované připojené klienty
    let client_connections = Arc::new(Mutex::new(std::collections::HashMap::<String, ClientConnection>::new()));
    
    // Hlavní smyčka zpracování zpráv
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
                    match signaling_msg.message_type.as_str() {
                        "register_success" => {
                            println!("[AudioServer] Registrace úspěšná");
                        },
                        "new_client" => {
                            if let Some(client_id) = signaling_msg.payload.get("client_id").and_then(|v| v.as_str()) {
                                println!("[AudioServer] Detekován nový klient: {}", client_id);
                            }
                        },
                        "offer" => {
                            if let Some(sender_id) = Some(signaling_msg.sender_id) {
                                println!("[AudioServer] Přijata nabídka od klienta: {}", sender_id);
                                
                                let ws_write_clone = Arc::clone(&ws_write);
                                let server_id_clone = server_id.clone();
                                let client_connections_clone = Arc::clone(&client_connections);
                                let client_id_clone = sender_id.clone();
                                
                                tokio::spawn(async move {
                                    if let Err(e) = handle_offer(
                                        &signaling_msg, 
                                        &client_id_clone,
                                        &server_id_clone, 
                                        ws_write_clone,
                                        client_connections_clone
                                    ).await {
                                        eprintln!("[AudioServer] Chyba při zpracování nabídky: {}", e);
                                    }
                                });
                            }
                        },
                        "ice_candidate" => {
                            if let Some(sender_id) = Some(signaling_msg.sender_id) {
                                let mut client_conns = client_connections.lock().await;
                                if let Some(conn) = client_conns.get_mut(&sender_id) {
                                    // Zpracovat ICE kandidáta
                                    if let Some(candidate) = signaling_msg.payload.get("candidate") {
                                        if let Some(sdp_mid) = signaling_msg.payload.get("sdpMid").and_then(|v| v.as_str()) {
                                            if let Some(sdp_mline_index) = signaling_msg.payload.get("sdpMLineIndex").and_then(|v| v.as_u64()) {
                                                if let Ok(candidate_str) = serde_json::to_string(candidate) {
                                                    let candidate_init = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                                                        candidate: candidate_str.trim_matches('"').to_string(),
                                                        sdp_mid: Some(sdp_mid.to_string()),
                                                        sdp_mline_index: Some(sdp_mline_index as u16),
                                                        username_fragment: None,
                                                    };
                                                    
                                                    if let Err(e) = conn.peer_connection.add_ice_candidate(candidate_init).await {
                                                        eprintln!("[AudioServer] Chyba při přidávání ICE kandidáta: {}", e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        _ => {
                            println!("[AudioServer] Neznámý typ zprávy: {}", signaling_msg.message_type);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("[AudioServer] Chyba při parsování zprávy: {}", e);
                }
            }
        }
    }
    
    println!("[AudioServer] Odpojeno od signalizačního serveru.");
    
    Ok(())
}

async fn handle_offer(
    msg: &SignalingMessage,
    client_id: &str,
    server_id: &str,
    ws_write: Arc<Mutex<futures_util::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>>>,
    client_connections: Arc<Mutex<std::collections::HashMap<String, ClientConnection>>>
) -> Result<(), Box<dyn std::error::Error>> {
    // Vytvoření MediaEngine
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs()?;
    
    // Konfigurace API
    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .build();
    
    // Konfigurace STUN serverů
    let mut rtc_config = RTCConfiguration::default();
    rtc_config.ice_servers = vec![
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            ..Default::default()
        },
    ];
    
    // Vytvoření peer connection
    let peer_connection = Arc::new(api.new_peer_connection(rtc_config).await?);
    
    // Zpracování ICE kandidátů
    let pc_clone = Arc::clone(&peer_connection);
    let ws_write_clone = Arc::clone(&ws_write);
    let server_id_clone = server_id.to_string();
    let client_id_clone = client_id.to_string();
    
    peer_connection.on_ice_candidate(Box::new(move |candidate| {
        let ws_write = ws_write_clone.clone();
        let server_id = server_id_clone.clone();
        let client_id = client_id_clone.clone();
        let pc = pc_clone.clone();
        
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_msg = SignalingMessage {
                    message_type: "ice_candidate".to_string(),
                    sender_id: server_id,
                    receiver_id: Some(client_id),
                    payload: serde_json::json!({
                        "candidate": candidate.to_json().unwrap(),
                        "sdpMid": candidate.sdp_mid,
                        "sdpMLineIndex": candidate.sdp_mline_index,
                    }),
                };
                
                if let Ok(msg_str) = serde_json::to_string(&candidate_msg) {
                    if let Err(e) = ws_write.lock().await.send(Message::text(msg_str)).await {
                        eprintln!("[AudioServer] Chyba při odesílání ICE kandidáta: {}", e);
                    }
                }
            }
        })
    })).await;
    
    // Nastavení data channel handleru
    let pc_clone = Arc::clone(&peer_connection);
    let client_id_for_dc = client_id.to_string();
    let client_connections_clone = Arc::clone(&client_connections);
    
    peer_connection.on_data_channel(Box::new(move |data_channel| {
        let client_id = client_id_for_dc.clone();
        let connections = client_connections_clone.clone();
        
        Box::pin(async move {
            println!("[AudioServer] Přijat data channel od klienta: {}", client_id);
            
            let dc_clone = Arc::clone(&data_channel);
            let client_id_clone = client_id.clone();
            
            data_channel.on_message(Box::new(move |msg| {
                let dc = dc_clone.clone();
                let client_id = client_id_clone.clone();
                
                Box::pin(async move {
                    if msg.is_binary() {
                        println!("[AudioServer] MIDI data od klienta {}: {:?}", client_id, msg.data);
                        
                        // Zde by byla implementace zpracování MIDI dat
                    }
                })
            }));
            
            // Uložení data channel do připojení klienta
            let mut connections_lock = connections.lock().await;
            if let Some(conn) = connections_lock.get_mut(&client_id) {
                conn.data_channel = Some(Arc::clone(&data_channel));
            }
        })
    }));
    
    // Nastavení handleru pro přijaté audio a video streamy
    let client_id_on_track = client_id.to_string();
    peer_connection.on_track(Box::new(move |track: Option<Arc<TrackRemote>>, _receiver| {
        let track_client_id = client_id_on_track.clone();
        
        Box::pin(async move {
            if let Some(track) = track {
                println!("[AudioServer] Přijat stream SSRC {} od klienta {}, typ: {}", track.ssrc(), track_client_id, track.kind());
                
                if track.kind() == webrtc::rtp_transceiver::rtp_codec::RTPCodecType::Audio {
                    tokio::spawn(async move {
                        let mut decoder = match Decoder::new(48000, Channels::Stereo) {
                            Ok(d) => d,
                            Err(e) => {
                                eprintln!("[AudioServer] Chyba při vytváření Opus dekodéru: {}", e);
                                return;
                            }
                        };
                        
                        let mut buffer = vec![0u8; 1500]; // Buffer pro RTP pakety
                        let mut decoded_buffer = vec![0.0f32; 960 * 2]; // 20ms stereo frame
                        
                        while let Ok((size, _)) = track.read(&mut buffer).await {
                            let opus_packet = &buffer[..size];
                            match decoder.decode_float(opus_packet, &mut decoded_buffer, false) {
                                Ok(num_samples) => {
                                    println!("[AudioServer] Dekódováno {} vzorků od klienta {}", num_samples, track_client_id);
                                    // Zde by byla implementace zpracování audia
                                },
                                Err(e) => {
                                    eprintln!("[AudioServer] Chyba při dekódování Opus: {}", e);
                                }
                            }
                        }
                        
                        println!("[AudioServer] Stream SSRC {} od klienta {} ukončen.", track.ssrc(), track_client_id);
                    });
                }
            }
        })
    }));
    
    // Stav připojení
    let client_id_for_state = client_id.to_string();
    let connections_for_state = Arc::clone(&client_connections);
    
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        let client_id = client_id_for_state.clone();
        let connections = connections_for_state.clone();
        
        Box::pin(async move {
            println!("[AudioServer] Stav připojení s klientem {} změněn na {:?}", client_id, state);
            
            if state == RTCPeerConnectionState::Failed || 
               state == RTCPeerConnectionState::Closed || 
               state == RTCPeerConnectionState::Disconnected {
                let mut connections_lock = connections.lock().await;
                connections_lock.remove(&client_id);
                println!("[AudioServer] Připojení s klientem {} uzavřeno", client_id);
            }
        })
    }));
    
    // Zpracování SDP nabídky
    if let Some(sdp) = msg.payload.get("sdp").and_then(|v| v.as_str()) {
        let offer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(sdp.to_string())?;
        peer_connection.set_remote_description(offer).await?;
        
        // Vytvoření odpovědi
        let answer = peer_connection.create_answer(None).await?;
        peer_connection.set_local_description(answer.clone()).await?;
        
        // Odeslání odpovědi klientovi
        let answer_msg = SignalingMessage {
            message_type: "answer".to_string(),
            sender_id: server_id.to_string(),
            receiver_id: Some(client_id.to_string()),
            payload: serde_json::json!({
                "sdp": peer_connection.local_description().await.unwrap().sdp,
                "type": "answer"
            }),
        };
        
        if let Ok(msg_str) = serde_json::to_string(&answer_msg) {
            ws_write.lock().await.send(Message::text(msg_str)).await?;
        }
        
        // Uložení připojení klienta
        let mut client_conns = client_connections.lock().await;
        client_conns.insert(client_id.to_string(), ClientConnection {
            peer_connection: Arc::clone(&peer_connection),
            data_channel: None,
        });
        
        println!("[AudioServer] Odpověď odeslána klientovi {}", client_id);
    }
    
    Ok(())
}