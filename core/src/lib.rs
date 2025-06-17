pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub mod event_bus;
pub mod mapping;
pub mod packet_processor;
pub mod journal_engine;

use std::fmt;

/// Chyba při práci se streamem
#[derive(Debug)]
pub enum StreamError {
    Io(std::io::Error),
    Network(String),
    Other(String),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::Io(e) => write!(f, "IO chyba: {}", e),
            StreamError::Network(s) => write!(f, "Síťová chyba: {}", s),
            StreamError::Other(s) => write!(f, "Jiná chyba: {}", s),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<std::io::Error> for StreamError {
    fn from(e: std::io::Error) -> Self {
        StreamError::Io(e)
    }
}

/// Trait pro odesílače datových streamů
pub trait DataStreamNetSender {
    fn init(&mut self) -> Result<(), StreamError>;
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError>;
    // Defaultní metoda pro sdílenou logiku (např. fragmentace)
    fn send_raw(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.send(ts, payload)
    }
}

/// Trait pro přijímače datových streamů
pub trait DataStreamNetReceiver {
    fn init(&mut self) -> Result<(), StreamError>;
    fn poll(&mut self, buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError>;
}

/// Mock implementace DataStreamNetSender pro testování a dependency injection
pub struct MockSender {
    pub sent: Vec<(u64, Vec<u8>)>,
}

impl MockSender {
    pub fn new() -> Self {
        Self { sent: Vec::new() }
    }
}

impl DataStreamNetSender for MockSender {
    fn init(&mut self) -> Result<(), StreamError> {
        Ok(())
    }
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.sent.push((ts, payload.to_vec()));
        Ok(())
    }
}

/*
Příklad použití v testu:

#[test]
fn test_sender_injection() {
    let mut sender = MockSender::new();
    sender.send(123, b"test").unwrap();
    assert_eq!(sender.sent.len(), 1);
    assert_eq!(sender.sent[0].1, b"test");
}
*/
