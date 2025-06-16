use ddp_rs::connection::DDPConnection;
use ddp_rs::protocol::PixelConfig;
use anyhow::Result;

/// Sends a frame of LED data to the DDP receiver (WLED).
pub fn send_ddp_frame(
    sender: &mut DDPConnection,
    data: &[u8],
) -> Result<()> {
    sender.write(data)?;
    Ok(())
}

/// Creates a DDPConnection for the given target IP, port, and pixel config.
/// Note: ddp-rs 0.3 only supports RGB (3 channels per pixel) via PixelConfig::default().
/// RGBW is not directly supported in this version of the crate.
pub fn create_ddp_sender(
    ip: &str,
    port: u16,
    led_count: usize,
    rgbw: bool, // This argument is kept for compatibility, but is not used.
) -> Result<DDPConnection> {
    // ddp-rs 0.3 does not support RGBW configuration. Only RGB is supported.
    // If you need RGBW, you must use a different crate or fork ddp-rs.
    let pixel_config = PixelConfig::default(); // Always RGB
    let addr = format!("{}:{}", ip, port);
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    let sender = DDPConnection::try_new(addr, pixel_config, ddp_rs::protocol::ID::Custom(1), socket)?;
    Ok(sender)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[test]
    fn test_create_ddp_sender_invalid_addr() {
        let res = create_ddp_sender("256.256.256.256", 4048, 10, false);
        assert!(res.is_err(), "Should fail for invalid IP");
    }

    // Note: For a real integration test, bind a UDP socket and check for received data.
} 