use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use url::Url;
use uuid::Uuid;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

use rtp_midi::{signaling_server::run_server, PeerType, signaling_server::RegisterPayload, signaling_server::SignalingMessage};

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