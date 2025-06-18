//! Android HAL adapter for DataStreamNetSender

use rtp_midi_core::{DataStreamNetSender, StreamError};

pub struct AndroidSender;

impl DataStreamNetSender for AndroidSender {
    fn init(&mut self) -> Result<(), StreamError> { Ok(()) }
    fn send(&mut self, _ts: u64, _payload: &[u8]) -> Result<(), StreamError> { Ok(()) }
}

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
