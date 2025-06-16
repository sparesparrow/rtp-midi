use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};
use webrtc::ice_transport::ice_server::RTCIceServer;
use opus::{Encoder, Channels, Application};
use url::Url;
use midir::{MidiInput, MidiInputPort, MidiInputConnection};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generování unikátního ID pro klienta
    let client_id = format!("client_{}", Uuid::new_v4());
    let signaling_url = "ws://localhost:8080/signaling";
    
    println!("[ClientApp {}] Připojování k signalizačnímu serveru...", client_id);
    
    let url = Url::parse(signaling_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    println!("[ClientApp {}] Připojeno k signalizačnímu serveru.", client_id);
    
    let (mut ws_write, mut ws_read) = ws_stream.split();
    
    // Registrace klienta
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
    
    // Čekání na potvrzení registrace
    let mut registered = false;
    let mut audio_server_id = String::new();
    
    while !registered {
        if let Some(msg) = ws_read.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("[ClientApp {}] Chyba při čtení zprávy: {}", client_id, e);
                    continue;
                }
            };
            
            if let Message::Text(text) = msg {
                if let Ok(signaling_msg) = serde_json::from_str::<SignalingMessage>(&text) {
                    if signaling_msg.message_type == "register_success" {
                        registered = true;
                        println!("[ClientApp {}] Registrace úspěšná", client_id);
                        
                        // Hledání audio serveru mezi připojenými klienty
                        if let Some(clients) = signaling_msg.payload.get("clients") {
                            if let Some(clients_array) = clients.as_array() {
                                for client in clients_array {
                                    if let (Some(id), Some(peer_type)) = (
                                        client[0].as_str(),
                                        client[1].as_str()
                                    ) {
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
        }
    }
    
    if audio_server_id.is_empty() {
        println!("[ClientApp {}] Audio server nenalezen. Ukončuji...", client_id);
        return Ok(());
    }
    
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
    
    // Vytvoření audio tracku
    let audio_track = Arc::new(TrackLocalStaticSample::new(
        webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_string(),
            ..Default::default()
        },
        "audio".to_string(),
        "audiosystem".to_string(),
    ));
    
    // Přidání audio tracku do peer connection
    let rtp_sender = peer_connection.add_track(Arc::clone(&audio_track) as Arc).await?;
    
    // Vytvoření data channel pro MIDI zprávy
    let midi_channel = peer_connection.create_data_channel(
        "midi", 
        None
    ).await?;
    
    let midi_channel_clone = Arc::clone(&midi_channel);
    midi_channel.on_open(Box::new(move || {
        println!("[ClientApp] MIDI data channel otevřen");
        Box::pin(async move {})
    }));
    
    // Zpracování ICE kandidátů
    let ws_write_clone = ws_write.clone();
    let client_id_clone = client_id.clone();
    let audio_server_id_clone = audio_server_id.clone();
    
    peer_connection.on_ice_candidate(Box::new(move |candidate| {
        let mut ws_write = ws_write_clone.clone();
        let client_id = client_id_clone.clone();
        let audio_server_id = audio_server_id_clone.clone();
        
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_msg = SignalingMessage {
                    message_type: "ice_candidate".to_string(),
                    sender_id: client_id,
                    receiver_id: Some(audio_server_id),
                    payload: serde_json::json!({
                        "candidate": candidate.to_json().unwrap(),
                        "sdpMid": candidate.sdp_mid,
                        "sdpMLineIndex": candidate.sdp_mline_index,
                    }),
                };
                
                if let Ok(msg_str) = serde_json::to_string(&candidate_msg) {
                    if let Err(e) = ws_write.send(Message::text(msg_str)).await {
                        eprintln!("[ClientApp] Chyba při odesílání ICE kandidáta: {}", e);
                    }
                }
            }
        })
    })).await;
    
    // Stav připojení
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        Box::pin(async move {
            println!("[ClientApp] Stav připojení změněn na {:?}", state);
        })
    }));
    
    // Vytvoření nabídky
    let offer = peer_connection.create_offer(None).await?;
    peer_connection.set_local_description(offer.clone()).await?;
    
    // Odeslání nabídky audio serveru
    let offer_msg = SignalingMessage {
        message_type: "offer".to_string(),
        sender_id: client_id.clone(),
        receiver_id: Some(audio_server_id.clone()),
        payload: serde_json::json!({
            "sdp": peer_connection.local_description().await.unwrap().sdp,
            "type": "offer"
        }),
    };
    
    let offer_msg_str = serde_json::to_string(&offer_msg)?;
    ws_write.send(Message::text(offer_msg_str)).await?;
    
    println!("[ClientApp {}] Nabídka odeslána audio serveru {}", client_id, audio_server_id);
    
    // Kanál pro synchronizaci ukončení
    let (done_tx, mut done_rx) = mpsc::channel::<()>(1);
    let done_tx_clone = done_tx.clone();
    
    // Spuštění generátoru sinusové vlny
    let audio_track_clone = Arc::clone(&audio_track);
    tokio::spawn(async move {
        let mut encoder = match Encoder::new(48000, Channels::Stereo, Application::Audio) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[ClientApp] Chyba při vytváření Opus enkodéru: {}", e);
                let _ = done_tx_clone.send(()).await;
                return;
            }
        };
        
        let mut phase = 0.0f32;
        let sample_rate = 48000.0;
        let frequency = 440.0;  // A4 tón
        let amplitude = 0.5;
        let samples_per_frame = 960;  // 20ms při 48kHz
        let mut sample_buf = vec![0.0f32; samples_per_frame * 2];  // Stereo
        let mut encoded_buf = vec![0u8; 1024];  // Buffer pro Opus pakety
        
        println!("[ClientApp] Spouštění odesílání sinusové vlny {} Hz", frequency);
        
        loop {
            // Generování sinusové vlny
            for i in 0..samples_per_frame {
                let sample = (phase * std::f32::consts::TAU).sin() * amplitude;
                sample_buf[i * 2] = sample;      // Levý kanál
                sample_buf[i * 2 + 1] = sample;  // Pravý kanál
                phase += frequency / sample_rate;
                if phase >= 1.0 { 
                    phase -= 1.0;
                }
            }
            
            // Enkódování audia do Opus formátu
            match encoder.encode_float(&sample_buf, &mut encoded_buf) {
                Ok(len) => {
                    let encoded_data = &encoded_buf[..len];
                    
                    // Vytvoření webrtc sample
                    let sample = webrtc::track::track_local::track_local_static_sample::Sample {
                        data: bytes::Bytes::copy_from_slice(encoded_data),
                        duration: Duration::from_millis(20),
                        ..Default::default()
                    };
                    
                    // Odeslání samplu
                    if let Err(e) = audio_track_clone.write_sample(&sample).await {
                        eprintln!("[ClientApp] Chyba při odesílání audio samplu: {}", e);
                        break;
                    }
                },
                Err(e) => {
                    eprintln!("[ClientApp] Chyba při enkódování Opus: {}", e);
                    break;
                }
            }
            
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    
    // Inicializace MIDI vstupu
    let midi_input = match MidiInput::new("AudioSystem Client") {
        Ok(midi) => midi,
        Err(e) => {
            eprintln!("[ClientApp] Chyba při inicializaci MIDI vstupu: {}", e);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "MIDI error")));
        }
    };
    
    // Výpis dostupných MIDI portů
    println!("[ClientApp] Dostupné MIDI vstupy:");
    for (i, port) in midi_input.ports().iter().enumerate() {
        println!("[{}] {}", i, midi_input.port_name(port).unwrap_or_else(|_| "Unknown".to_string()));
    }
    
    // Pokud jsou dostupné MIDI porty, připojíme se k prvnímu
    if !midi_input.ports().is_empty() {
        let midi_port = midi_input.ports()[0].clone();
        let midi_channel_for_midi = Arc::clone(&midi_channel);
        
        let _midi_connection = midi_input.connect(
            &midi_port, 
            "midi-connection", 
            move |_timestamp, data, _| {
                println!("[ClientApp] MIDI zpráva přijata: {:?}", data);
                
                // Odeslání MIDI dat přes data channel
                if midi_channel_for_midi.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open {
                    if let Err(e) = midi_channel_for_midi.send_with_binary(bytes::Bytes::copy_from_slice(data)) {
                        eprintln!("[ClientApp] Chyba při odesílání MIDI dat: {}", e);
                    }
                }
            },
            (),
        )?;
        
        println!("[ClientApp] MIDI vstup připojen");
    } else {
        println!("[ClientApp] Žádné MIDI vstupy nenalezeny");
    }
    
    // Zpracování zpráv ze signalizačního serveru
    let mut ice_gathering_complete = false;
    let pc_clone = Arc::clone(&peer_connection);
    
    tokio::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("[ClientApp] Chyba při čtení zprávy: {}", e);
                    break;
                }
            };
            
            if let Message::Text(text) = msg {
                match serde_json::from_str::<SignalingMessage>(&text) {
                    Ok(signaling_msg) => {
                        match signaling_msg.message_type.as_str() {
                            "answer" => {
                                println!("[ClientApp] Přijata odpověď od audio serveru");
                                
                                if let Some(sdp) = signaling_msg.payload.get("sdp").and_then(|v| v.as_str()) {
                                    let answer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::answer(sdp.to_string()).unwrap();
                                    if let Err(e) = pc_clone.set_remote_description(answer).await {
                                        eprintln!("[ClientApp] Chyba při nastavování remote description: {}", e);
                                    } else {
                                        println!("[ClientApp] Remote description nastavena");
                                        
                                        // Nastavíme flag pro kompletní ICE gathering, pokud už byly kandidáti shromážděni
                                        if ice_gathering_complete {
                                            done_tx.send(()).await.ok();
                                        }
                                    }
                                }
                            },
                            "ice_candidate" => {
                                println!("[ClientApp] Přijat ICE kandidát od audio serveru");
                                
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
                                                
                                                if let Err(e) = pc_clone.add_ice_candidate(candidate_init).await {
                                                    eprintln!("[ClientApp] Chyba při přidávání ICE kandidáta: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            _ => {
                                println!("[ClientApp] Neznámý typ zprávy: {}", signaling_msg.message_type);
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("[ClientApp] Chyba při parsování zprávy: {}", e);
                    }
                }
            }
        }
        
        // Signál pro ukončení
        done_tx.send(()).await.ok();
    });
    
    // Čekání na ukončení
    done_rx.recv().await;
    
    // Čekání před ukončením, aby se stihly odeslat všechny zprávy
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    println!("[ClientApp {}] Aplikace ukončena", client_id);
    
    Ok(())
}