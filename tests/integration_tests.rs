use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use url::Url;
use uuid::Uuid;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

use rtp_midi::{signaling_server::run_server, PeerType, signaling_server::RegisterPayload, signaling_server::SignalingMessage};
use rtp_midi::midi::message::MidiMessage;
use rtp_midi::midi::rtp::RtpMidiPacket;
use rtp_midi::{map_audio_to_leds};
use rtp_midi::wled_control;
use mockito;
use output::light_mapper::{map_leds_with_preset, MappingPreset};


#[tokio::test]
async fn test_client_registration() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Bind a TcpListener to a random port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    println!("Test server listening on: {}", addr);

    // 2. Spawn the signaling server as a background task
    tokio::spawn(async move {
        run_server(listener).await
    });

    // Give the server a moment to start up
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Connect a WebSocket client to the server
    let ws_url = format!("ws://{}/signaling", addr);
    let url = Url::parse(&ws_url)?;
    let (ws_stream, _) = connect_async(url.as_str()).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    // 4. Send a register message from this test client
    let client_id = format!("test_client_{}", Uuid::new_v4());
    let register_payload = RegisterPayload {
        peer_type: PeerType::ClientApp,
        client_id: client_id.clone(),
    };
    let register_msg = SignalingMessage {
        message_type: "register".to_string(),
        sender_id: client_id.clone(),
        receiver_id: None,
        payload: json!(register_payload),
    };

    let register_msg_str = serde_json::to_string(&register_msg)?;
    ws_write.send(tokio_tungstenite::tungstenite::Message::text(register_msg_str)).await?;

    // 5. Assert that the client receives a register_success message in response
    let response_msg = ws_read.next().await.expect("Did not receive a response").unwrap();
    assert!(response_msg.is_text());

    let response_text = response_msg.to_text()?;
    let signaling_response: SignalingMessage = serde_json::from_str(response_text)?;

    assert_eq!(signaling_response.message_type, "register_success");
    assert_eq!(signaling_response.payload["registered_id"], client_id);

    println!("Client successfully registered and received register_success.");

    Ok(())
}

#[test]
fn test_rtp_midi_packet_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    // Tento test ověřuje, že RTP-MIDI paket lze serializovat a deserializovat bez ztráty dat.
    // Je to klíčové pro zajištění spolehlivosti síťové komunikace.

    // 1. Vytvoření vzorové MIDI zprávy
    let midi_note_on = MidiMessage::new(0, vec![0x90, 60, 127]); // Note On, Channel 1, Middle C, full velocity
    let mut original_packet = RtpMidiPacket::create(vec![midi_note_on]);
    original_packet.set_sequence_number(12345);
    original_packet.set_ssrc(54321);

    // 2. Serializace paketu
    let serialized_data = original_packet.serialize()?;

    // 3. Deserializace dat zpět na paket
    let parsed_packet = RtpMidiPacket::parse(&serialized_data)?;

    // 4. Ověření, že jsou pakety shodné
    assert_eq!(original_packet.sequence_number(), parsed_packet.sequence_number());
    assert_eq!(original_packet.ssrc(), parsed_packet.ssrc());
    assert_eq!(original_packet.midi_commands(), parsed_packet.midi_commands());
    assert_eq!(original_packet.journal_present(), parsed_packet.journal_present());

    Ok(())
}

#[tokio::test]
async fn test_wled_preset_control_mocked() -> Result<(), Box<dyn std::error::Error>> {
    // Tento test používá mock server k simulaci WLED zařízení.
    // Ověřuje, že funkce pro ovládání WLED odesílá správně formátované HTTP požadavky.

    // 1. Spuštění mock serveru na náhodném portu
    let mut server = mockito::Server::new_async().await;
    let url = server.url(); // Např. "http://127.0.0.1:1234"

    // 2. Vytvoření mocku pro WLED JSON API endpoint
    let mock = server.mock("POST", "/json/state")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success":true}"#)
        .expect(1) // Očekáváme přesně jeden požadavek
        .with_body(r#"{"ps":5}"#) // Očekáváme toto konkrétní JSON tělo
        .create_async().await;

    // 3. Zavolání naší funkce, která má odeslat HTTP požadavek
    // Adresa serveru se předává bez "http://"
    let ip_and_port = url.strip_prefix("http://").unwrap();
    wled_control::set_wled_preset(ip_and_port, 5).await?;

    // 4. Ověření, že mock byl skutečně zavolán
    mock.assert_async().await;

    Ok(())
}


#[test]
fn test_audio_to_led_pipeline() {
    // Tento test ověřuje celý řetězec zpracování audia na LED barvy.

    // 1. Vytvoření vzorového zvukového signálu (magnitud)
    // Simulujeme silné basy, slabé středy a žádné výšky.
    let mut magnitudes = vec![0.0; 90];
    // Silné basy (první třetina)
    for i in 0..30 { magnitudes[i] = 0.9; }
    // Slabé středy (druhá třetina)
    for i in 30..60 { magnitudes[i] = 0.2; }
    // Žádné výšky (třetí třetina)

    let led_count = 10;

    // 2. Spuštění mapovací funkce
    let led_data = map_audio_to_leds(&magnitudes, led_count);

    // 3. Ověření výsledku
    // Očekávané hodnoty: R = 0.9*255=229, G = 0.2*255=51, B = 0.0*255=0
    assert_eq!(led_data.len(), led_count * 3, "Incorrect number of LED data bytes");
    for i in 0..led_count {
        let r = led_data[i * 3];
        let g = led_data[i * 3 + 1];
        let b = led_data[i * 3 + 2];
        assert_eq!(r, 229, "Red component mismatch at LED {}", i);
        assert_eq!(g, 51, "Green component mismatch at LED {}", i);
        assert_eq!(b, 0, "Blue component mismatch at LED {}", i);
    }
}

#[test]
fn test_audio_to_led_end_to_end_presets() {
    // Simulate a strong audio signal (all bands)
    let magnitudes = vec![1.0; 16];
    let led_count = 8;
    // Spectrum preset
    let spectrum_leds = map_leds_with_preset(&magnitudes, led_count, MappingPreset::Spectrum);
    assert_eq!(spectrum_leds.len(), led_count * 3);
    // Should be bright colors (not all zero)
    assert!(spectrum_leds.iter().any(|&v| v > 0));
    // VuMeter preset
    let vumeter_leds = map_leds_with_preset(&magnitudes, led_count, MappingPreset::VuMeter);
    // All LEDs should be green (0,255,0)
    for i in 0..led_count {
        assert_eq!(vumeter_leds[i*3], 0);
        assert_eq!(vumeter_leds[i*3+1], 255);
        assert_eq!(vumeter_leds[i*3+2], 0);
    }
}
