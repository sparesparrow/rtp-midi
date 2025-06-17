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

/// Chyba při práci se streamem
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("IO chyba: {0}")]
    Io(#[from] std::io::Error),
    #[error("Síťová chyba: {0}")]
    Network(String),
    #[error("Jiná chyba: {0}")]
    Other(String),
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
