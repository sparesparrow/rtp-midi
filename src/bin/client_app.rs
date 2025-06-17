use anyhow::Result;
use bytes::Bytes;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use midir::{MidiInput, MidiInputConnection};
use opus::{Application, Channels, Encoder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use url::Url;
use uuid::Uuid;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

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

#[tokio::main]
async fn main() -> Result<()> {
    let client_id = format!("client_{}", Uuid::new_v4());
    let signaling_url = "ws://localhost:8080/signaling";

    println!("[ClientApp {}] Připojování k signalizačnímu serveru...", client_id);

    let url = Url::parse(signaling_url)?;
    let (ws_stream, _) = connect_async(url.as_str()).await?;
    println!("[ClientApp {}] Připojeno k signalizačnímu serveru.", client_id);

    let (mut ws_write, mut ws_read) = ws_stream.split();

    let register_payload = RegisterPayload {
        peer_type: PeerType::ClientApp,
        client_id: client_id.clone(),
    };

    let register_msg = SignalingMessage {
        message_type: "register".to_string(),
        sender_id: client_id.clone(),
        receiver_id: None,
        payload: serde_json::to_value(register_payload)?,
    };

    let register_msg_str = serde_json::to_string(&register_msg)?;
    ws_write.send(Message::text(register_msg_str)).await?;

    let mut registered = false;
    let mut audio_server_id = String::new();

    while !registered {
        if let Some(Ok(Message::Text(text))) = ws_read.next().await {
            if let Ok(signaling_msg) = serde_json::from_str::<SignalingMessage>(&text) {
                if signaling_msg.message_type == "register_success" {
                    registered = true;
                    println!("[ClientApp {}] Registrace úspěšná", client_id);

                    if let Some(clients) = signaling_msg.payload.get("clients").and_then(|c| c.as_array()) {
                        for client in clients {
                            if let (Some(id), Some(peer_type)) =
                                (client.get("client_id").and_then(|i| i.as_str()), client.get("peer_type").and_then(|p| p.as_str()))
                            {
                                if peer_type == "AudioServer" {
                                    audio_server_id = id.to_string();
                                    println!("[ClientApp {}] Nalezen audio server: {}", client_id, audio_server_id);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if audio_server_id.is_empty() {
        println!("[ClientApp {}] Audio server nenalezen. Ukončuji...", client_id);
        return Ok(());
    }

    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs()?;
    let api = APIBuilder::new().with_media_engine(media_engine).build();
    let mut rtc_config = RTCConfiguration::default();
    rtc_config.ice_servers = vec![RTCIceServer {
        urls: vec!["stun:stun.l.google.com:19302".to_string()],
        ..Default::default()
    }];
    let peer_connection: Arc<RTCPeerConnection> = Arc::new(api.new_peer_connection(rtc_config).await?);
    
    let audio_track: Arc<TrackLocalStaticSample> = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_string(),
            ..Default::default()
        },
        "audio".to_string(),
        "audiosystem".to_string(),
    ));
    
    let _rtp_sender = peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;
    
    let midi_channel: Arc<RTCDataChannel> = peer_connection.create_data_channel("midi", None).await?;
    let midi_channel_clone = Arc::clone(&midi_channel);
    midi_channel.on_open(Box::new(move || {
        println!("[ClientApp] MIDI data channel otevřen");
        Box::pin(async move {})
    }));
    
    let ws_write_clone = Arc::new(Mutex::new(ws_write));
    let client_id_clone = client_id.clone();
    let audio_server_id_clone = audio_server_id.clone();
    
    peer_connection
        .on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidateInit>| {
            let ws_write = Arc::clone(&ws_write_clone);
            let client_id = client_id_clone.clone();
            let audio_server_id = audio_server_id_clone.clone();
    
            Box::pin(async move {
                if let Some(candidate) = candidate {
                    let candidate_msg = SignalingMessage {
                        message_type: "ice_candidate".to_string(),
                        sender_id: client_id,
                        receiver_id: Some(audio_server_id),
                        payload: json!({ "candidate": candidate }),
                    };
    
                    if let Ok(msg_str) = serde_json::to_string(&candidate_msg) {
                        if let Err(e) = ws_write.lock().await.send(Message::text(msg_str)).await {
                            eprintln!("[ClientApp] Chyba při odesílání ICE kandidáta: {}", e);
                        }
                    }
                }
            })
        }))
        .await;
    
    peer_connection
        .on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            Box::pin(async move {
                println!("[ClientApp] Stav připojení změněn na {:?}", state);
            })
        }))
        .await;
    
    let offer = peer_connection.create_offer(None).await?;
    peer_connection.set_local_description(offer.clone()).await?;
    
    let offer_msg = SignalingMessage {
        message_type: "offer".to_string(),
        sender_id: client_id.clone(),
        receiver_id: Some(audio_server_id.clone()),
        payload: json!({ "sdp": peer_connection.local_description().await.unwrap().sdp, "type": "offer" }),
    };
    
    let offer_msg_str = serde_json::to_string(&offer_msg)?;
    ws_write_clone.lock().await.send(Message::text(offer_msg_str)).await?;
    println!("[ClientApp {}] Nabídka odeslána audio serveru {}", client_id, audio_server_id);
    
    Ok(())
}