use anyhow::Result;
use ddp_rs::connection::DDPConnection;
use core::{DataStreamNetSender, StreamError, DataStreamNetReceiver};

/// Wrapper pro DDP odesílač implementující sjednocené API
pub struct DdpSender {
    conn: DDPConnection,
}

impl DdpSender {
    pub fn new(conn: DDPConnection) -> Self {
        Self { conn }
    }
}

impl DataStreamNetSender for DdpSender {
    fn init(&mut self) -> Result<(), StreamError> {
        // DDPConnection je již inicializován
        Ok(())
    }
    fn send(&mut self, _ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.conn.write(payload).map_err(|e| StreamError::Other(e.to_string()))
    }
}

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

/// Šablona pro DDP přijímač implementující sjednocené API
pub struct DdpReceiver {
    // zde bude např. socket nebo jiný zdroj
}

impl DdpReceiver {
    pub fn new() -> Self {
        Self { }
    }
}

impl DataStreamNetReceiver for DdpReceiver {
    fn init(&mut self) -> Result<(), StreamError> {
        // TODO: Inicializace přijímače (např. otevření socketu)
        Ok(())
    }
    fn poll(&mut self, _buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError> {
        // TODO: Čtení dat z DDP streamu
        Ok(None)
    }
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