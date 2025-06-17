use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use log::error;
use crate::mapping::WledOutputAction;

/// Sets a WLED preset by ID via the JSON API.
pub async fn set_wled_preset(wled_ip: &str, preset_id: i32) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);
    let payload = json!({ "ps": preset_id });
    let res = client.post(&url).json(&payload).send().await?;
    if !res.status().is_success() {
        anyhow::bail!("WLED preset set failed: {}", res.status());
    }
    Ok(())
}

/// Sets the WLED color (primary segment) via the JSON API.
pub async fn set_wled_color(wled_ip: &str, r: u8, g: u8, b: u8) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);
    let payload = json!({ "seg": [{ "col": [[r, g, b]] }] });
    let res = client.post(&url).json(&payload).send().await?;
    if !res.status().is_success() {
        anyhow::bail!("WLED color set failed: {}", res.status());
    }
    Ok(())
}

/// Sets the overall WLED brightness via the JSON API.
pub async fn set_wled_brightness(wled_ip: &str, brightness: u8) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);
    let payload = json!({ "bri": brightness });
    let res = client.post(&url).json(&payload).send().await?;
    if !res.status().is_success() {
        anyhow::bail!("WLED brightness set failed: {}", res.status());
    }
    Ok(())
}

/// Sets a WLED effect for the primary segment via the JSON API.
pub async fn set_wled_effect(wled_ip: &str, effect_id: i32, speed: Option<u8>, intensity: Option<u8>) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);
    
    let mut seg_payload = serde_json::Map::new();
    seg_payload.insert("fx".to_string(), json!(effect_id));
    if let Some(s) = speed { seg_payload.insert("sx".to_string(), json!(s)); }
    if let Some(i) = intensity { seg_payload.insert("ix".to_string(), json!(i)); }

    let payload = json!({ "seg": [seg_payload] });
    let res = client.post(&url).json(&payload).send().await?;
    if !res.status().is_success() {
        anyhow::bail!("WLED effect set failed: {}", res.status());
    }
    Ok(())
}

/// Sets a WLED color palette for the primary segment via the JSON API.
pub async fn set_wled_palette(wled_ip: &str, palette_id: i32) -> Result<()> {
    let client = Client::new();
    let url = format!("http://{}/json/state", wled_ip);
    let payload = json!({ "seg": [{ "pal": palette_id }] });
    let res = client.post(&url).json(&payload).send().await?;
    if !res.status().is_success() {
        anyhow::bail!("WLED palette set failed: {}", res.status());
    }
    Ok(())
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