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

/// Chyba při práci se streamem.
/// 
/// Používá se ve všech implementacích DataStreamNetSender/Receiver pro sjednocené API napříč platformami.
#[derive(Debug)]
pub enum StreamError {
    /// IO chyba (např. socket, soubor)
    Io(std::io::Error),
    /// Síťová chyba (např. unreachable, timeout)
    Network(String),
    /// Ostatní chyby
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

/// Trait pro odesílače datových streamů (síť, HW, ...).
/// 
/// Implementujte pro každý typ výstupu (WLED, DDP, DMX, ...).
/// Umožňuje jednotné API napříč platformami a buildy.
pub trait DataStreamNetSender {
    /// Inicializace zařízení/zdroje (volitelné, lze nechat prázdné)
    fn init(&mut self) -> Result<(), StreamError>;
    /// Odeslání datového paketu s timestampem
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError>;
    /// Defaultní metoda pro sdílenou logiku (např. fragmentace, retry)
    fn send_raw(&mut self, ts: u64, payload: &[u8]) -> Result<(), StreamError> {
        self.send(ts, payload)
    }
}

/// Trait pro přijímače datových streamů (síť, HW, ...).
/// 
/// Implementujte pro každý typ vstupu (RTP-MIDI, DDP, ...).
pub trait DataStreamNetReceiver {
    /// Inicializace přijímače
    fn init(&mut self) -> Result<(), StreamError>;
    /// Čtení/polling dat do bufferu, vrací timestamp a délku
    fn poll(&mut self, buf: &mut [u8]) -> Result<Option<(u64, usize)>, StreamError>;
}

/// Mock implementace DataStreamNetSender pro testování a dependency injection.
/// 
/// Umožňuje testovat logiku bez skutečného síťového/hardware výstupu.
/// Příklad použití v testu viz níže.
pub struct MockSender {
    pub sent: Vec<(u64, Vec<u8>)>,
}

impl Default for MockSender {
    fn default() -> Self {
        Self::new()
    }
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

pub use crate::journal_engine::{JournalData, JournalEntry};
