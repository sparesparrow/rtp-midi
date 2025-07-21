use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use serde_json::json;
// use utils::NetworkInterface; // Removed to break dependency cycle
use rtp_midi_core::WledOutputAction;
use rtp_midi_core::{DataStreamNetSender, StreamError};

/// Wrapper pro WLED JSON API odesílač implementující sjednocené API.
///
/// Umožňuje odesílat příkazy na WLED zařízení přes HTTP/JSON jednotným způsobem (implementace DataStreamNetSender).
/// Použijte např. v service loop nebo v enum dispatch pro embedded buildy.
///
/// Příklad použití:
/// let mut sender = WledSender::new("192.168.1.100".to_string());
/// sender.send(0, br#"{\"bri\":128}"#);
pub struct WledSender {
    ip: String,
}

impl WledSender {
    pub fn new(ip: String) -> Self {
        Self { ip }
    }
}

impl DataStreamNetSender for WledSender {
    fn init(&mut self) -> Result<(), StreamError> {
        // Není potřeba žádná speciální inicializace
        Ok(())
    }
    fn send(&mut self, _ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        // Payload je JSON (nebo serializovaný příkaz)
        let json_payload = match serde_json::from_slice::<serde_json::Value>(payload) {
            Ok(val) => val,
            Err(e) => return Err(StreamError::Other(format!("Invalid JSON payload: {e}"))),
        };
        // Blokující verze pro jednoduchost (v produkci by bylo async)
        let client = reqwest::blocking::Client::new();
        let url = format!("http://{}/json/state", self.ip);
        let resp = client.post(&url).json(&json_payload).send();
        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(StreamError::Other(format!(
                "WLED HTTP error: {}",
                r.status()
            ))),
            Err(e) => Err(StreamError::Other(format!("WLED send error: {e}"))),
        }
    }
}

/// Sends a generic JSON command to the WLED device.
pub async fn send_wled_json_command(wled_ip: &str, json_payload: serde_json::Value) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{wled_ip}/json/state");

    match client.post(&url).json(&json_payload).send().await {
        Ok(response) => {
            if response.status().is_success() {
                info!(
                    "Successfully sent WLED command. Status: {}",
                    response.status()
                );
            } else {
                error!(
                    "Failed to send WLED command. Status: {}, Response: {:?}",
                    response.status(),
                    response.text().await
                );
            }
        }
        Err(e) => error!("Error sending WLED command: {e}"),
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
pub async fn set_wled_effect(
    wled_ip: &str,
    effect_id: i32,
    speed: Option<u8>,
    intensity: Option<u8>,
) -> Result<()> {
    let mut seg_payload = serde_json::Map::new();
    seg_payload.insert("fx".to_string(), json!(effect_id));
    if let Some(s) = speed {
        seg_payload.insert("sx".to_string(), json!(s));
    }
    if let Some(i) = intensity {
        seg_payload.insert("ix".to_string(), json!(i));
    }

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
            set_wled_preset(wled_ip, (*id).try_into().unwrap()).await
        }
        WledOutputAction::SetBrightness { value } => set_wled_brightness(wled_ip, *value).await,
        WledOutputAction::SetColor { r, g, b } => set_wled_color(wled_ip, *r, *g, *b).await,
        WledOutputAction::SetEffect {
            id,
            speed,
            intensity,
        } => set_wled_effect(wled_ip, *id, *speed, *intensity).await,
        WledOutputAction::SetPalette { id } => set_wled_palette(wled_ip, *id).await,
    };

    if let Err(e) = result {
        error!("Failed to execute WLED action {action:?}: {e}");
    }
}
