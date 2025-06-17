use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use log::{error, info};
use crate::event_bus::Event;
use crate::network_interface::NetworkInterface;
use crate::rtp_midi_lib::UdpSocket;
use std::sync::Arc;
use tokio::sync::broadcast;
use std::net::SocketAddr;
use crate::mapping::WledOutputAction;

/// Sends a generic JSON command to the WLED device.
pub async fn send_wled_json_command(
    wled_ip: &str,
    json_payload: serde_json::Value,
) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);

    match client.post(&url).json(&json_payload).send().await {
        Ok(response) => {
            if response.status().is_success() {
                info!("Successfully sent WLED command. Status: {}", response.status());
            } else {
                error!("Failed to send WLED command. Status: {}, Response: {:?}", response.status(), response.text().await);
            }
        },
        Err(e) => error!("Error sending WLED command: {}", e),
    }
    Ok(())
}

/// Sets the overall WLED brightness via the JSON API.
pub async fn set_wled_brightness(wled_ip: &str, brightness: u8) -> Result<()> {
    let payload = json!({ "bri": brightness });
    send_wled_json_command(wled_ip, payload).await
}

/// Sets the WLED color (primary segment) via the JSON API.
pub async fn set_wled_color(wled_ip: &str, r: u8, g: u8, b: u8) -> Result<()> {
    let payload = json!({
        "seg": [{
            "col": [r, g, b]
        }]
    });
    send_wled_json_command(wled_ip, payload).await
}

/// Sets a WLED preset by ID via the JSON API.
pub async fn set_wled_preset(wled_ip: &str, preset_id: u8) -> Result<()> {
    let payload = json!({ "ps": preset_id });
    send_wled_json_command(wled_ip, payload).await
}

/// Sets a WLED effect for the primary segment via the JSON API.
pub async fn set_wled_effect(wled_ip: &str, effect_id: i32, speed: Option<u8>, intensity: Option<u8>) -> Result<()> {
    let mut seg_payload = serde_json::Map::new();
    seg_payload.insert("fx".to_string(), json!(effect_id));
    if let Some(s) = speed { seg_payload.insert("sx".to_string(), json!(s)); }
    if let Some(i) = intensity { seg_payload.insert("ix".to_string(), json!(i)); }

    let payload = json!({ "seg": [seg_payload] });
    send_wled_json_command(wled_ip, payload).await
}

/// Sets a WLED color palette for the primary segment via the JSON API.
pub async fn set_wled_palette(wled_ip: &str, palette_id: i32) -> Result<()> {
    let payload = json!({ "seg": [{ "pal": palette_id }] });
    send_wled_json_command(wled_ip, payload).await
}

/// Toggles the WLED power state via the JSON API.
pub async fn toggle_wled_power(wled_ip: &str) -> Result<()> {
    let payload = json!({ "on": "t" });
    send_wled_json_command(wled_ip, payload).await
}

pub async fn execute_wled_action(action: &WledOutputAction, wled_ip: &str) {
    let result = match action {
        WledOutputAction::SetPreset { id } => {
            set_wled_preset(wled_ip, *id).await
        }
        WledOutputAction::SetBrightness { value } => {
            set_wled_brightness(wled_ip, *value).await
        }
        WledOutputAction::SetColor { r, g, b } => {
            set_wled_color(wled_ip, *r, *g, *b).await
        }
        WledOutputAction::SetEffect { id, speed, intensity } => {
            set_wled_effect(wled_ip, *id, *speed, *intensity).await
        }
        WledOutputAction::SetPalette { id } => {
            set_wled_palette(wled_ip, *id).await
        }
    };

    if let Err(e) = result {
        error!("Failed to execute WLED action {:?}: {}", action, e);
    }
} 