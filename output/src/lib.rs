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

pub mod wled_control;
pub mod ddp_output;
pub mod light_mapper;

#[cfg(feature = "hal_esp32")]
use crate::wled_control::WledSender;
#[cfg(feature = "hal_esp32")]
use crate::ddp_output::DdpSender;
#[cfg(feature = "hal_esp32")]
use core::DataStreamNetSender;
#[cfg(feature = "hal_esp32")]

/// Enum dispatch pro embedded buildy (ESP32).
/// 
/// Umožňuje staticky vybírat mezi různými výstupy (WLED, DDP, ...),
/// což vede k menší binárce a žádnému RTTI na embedded platformách.
/// 
/// Příklad použití:
/// #[cfg(feature = "hal_esp32")]
/// let mut sender = StreamSender::Wled(WledSender::new("192.168.1.100".to_string()));
/// sender.send(0, b"{\"bri\":128}").unwrap();
#[cfg(feature = "hal_esp32")]
pub enum StreamSender {
    Wled(WledSender),
    Ddp(DdpSender),
    // Přidejte další varianty podle potřeby
}

#[cfg(feature = "hal_esp32")]
impl DataStreamNetSender for StreamSender {
    fn init(&mut self) -> Result<(), core::StreamError> {
        match self {
            StreamSender::Wled(s) => s.init(),
            StreamSender::Ddp(s) => s.init(),
        }
    }
    fn send(&mut self, ts: u64, payload: &[u8]) -> Result<(), core::StreamError> {
        match self {
            StreamSender::Wled(s) => s.send(ts, payload),
            StreamSender::Ddp(s) => s.send(ts, payload),
        }
    }
}

/*
Příklad použití na embedded platformě:

#[cfg(feature = "hal_esp32")]
let mut sender = StreamSender::Wled(WledSender::new("192.168.1.100".to_string()));
sender.send(0, b"{\"bri\":128}").unwrap();
*/
