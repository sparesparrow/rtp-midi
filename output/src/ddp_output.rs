use anyhow::Result;
use ddp_rs::connection::DDPConnection;

/// Sends a frame of LED data to the DDP receiver (WLED).
pub fn send_ddp_frame(sender: &mut DDPConnection, data: &[u8]) -> Result<()> {
    sender.write(data)?;
    Ok(())
}

pub fn create_ddp_sender(ip: &str, port: u16, _led_count: usize, _rgbw: bool) -> Result<DDPConnection> {
    let pixel_config = ddp_rs::protocol::PixelConfig::default(); // Always RGB
    let addr = format!("{}:{}", ip, port);
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    let sender = DDPConnection::try_new(addr, pixel_config, ddp_rs::protocol::ID::Custom(1), socket)?;
    Ok(sender)
}

#[cfg(test)]
mod ddp_tests {
    use super::*;

    #[test]
    fn test_create_ddp_sender_valid_config() {
        let result = create_ddp_sender("127.0.0.1", 4048, 10, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_ddp_sender_invalid_addr() {
        // This test might fail if the address is somehow resolvable or if DDPConnection handles it gracefully
        // For now, testing for is_ok() on valid cases and is_err() on clearly invalid ones.
        let result = create_ddp_sender("invalid-ip", 4048, 10, false);
        assert!(result.is_err());
    }

    // Note: send_ddp_frame is hard to test in isolation without a mock UDP socket or a real receiver.
    // It's better covered by integration tests.
} 