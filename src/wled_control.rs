use anyhow::Result;
use reqwest::Client;
use serde_json::json;

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