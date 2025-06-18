use anyhow::Result;
use ddp_rs::connection::DDPConnection;
use rtp_midi_core::{DataStreamNetSender, StreamError, DataStreamNetReceiver};

/// Wrapper pro DDP odesílač implementující sjednocené API.
/// 
/// Umožňuje odesílat LED data přes DDP protokol jednotným způsobem (implementace DataStreamNetSender).
/// Použijte např. v service loop nebo v enum dispatch pro embedded buildy.
///
/// Příklad použití:
/// let mut sender = DdpSender::new(ddp_conn);
/// sender.send(0, &[0,1,2,3]);
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
        self.conn.write(payload)
            .map(|_| ())
            .map_err(|e| StreamError::Other(e.to_string()))
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

/// Šablona pro DDP přijímač implementující sjednocené API.
/// 
/// Připravena pro budoucí rozšíření (implementace DataStreamNetReceiver).
///
/// Příklad použití:
/// let mut rx = DdpReceiver::new();
/// rx.init()?;
/// let mut buf = [0u8; 512];
/// if let Some((ts, len)) = rx.poll(&mut buf)? { /* ... */ }
pub struct DdpReceiver {
    socket: Option<std::net::UdpSocket>,
}

impl Default for DdpReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl DdpReceiver {
    pub fn new() -> Self {
        Self { socket: None }
    }
}

impl DataStreamNetReceiver for DdpReceiver {
    fn init(&mut self) -> Result<(), StreamError> {
        // Inicializace přijímače (otevření socketu na DDP portu 4048)
        let socket = std::net::UdpSocket::bind("0.0.0.0:4048")
            .map_err(|e| StreamError::Other(format!("Failed to bind DDP socket: {}", e)))?;
        socket.set_nonblocking(true)
            .map_err(|e| StreamError::Other(format!("Failed to set non-blocking: {}", e)))?;
        self.socket = Some(socket);
        Ok(())
    }
    fn poll(&mut self, buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError> {
        if let Some(socket) = &self.socket {
            match socket.recv(buf) {
                Ok(len) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| StreamError::Other(format!("Time error: {}", e)))?
                        .as_millis() as u64;
                    Ok(Some((ts, len)))
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(e) => Err(StreamError::Other(format!("DDP recv error: {}", e))),
            }
        } else {
            Err(StreamError::Other("DDP receiver socket not initialized".to_string()))
        }
    }
}

// DdpReceiver is now fully implemented and ready for integration with the event bus and service loop.
// It supports non-blocking UDP reads on port 4048 and returns timestamped data frames.
// To use, instantiate DdpReceiver, call init(), and poll() in your main loop or async task.

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