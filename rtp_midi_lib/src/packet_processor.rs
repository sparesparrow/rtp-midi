use anyhow::{anyhow, Result};
use bytes::Buf;

/// Represents the parsed components of a raw network packet.
pub struct ParsedPacket {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Vec<u8>,
}

impl ParsedPacket {
    /// Parses a raw byte slice into a ParsedPacket, extracting RTP header and payload.
    pub fn parse_rtp_packet(data: &[u8]) -> Result<Self> {
        let mut reader = bytes::Bytes::copy_from_slice(data);
        if reader.len() < 12 {
            return Err(anyhow!("RTP header too short"));
        }

        let byte0 = reader.get_u8();
        let version = (byte0 >> 6) & 0x03;
        if version != 2 {
            return Err(anyhow!("Unsupported RTP version: {}", version));
        }
        let padding = (byte0 >> 5) & 0x01 == 1;
        let extension = (byte0 >> 4) & 0x01 == 1;
        
        let byte1 = reader.get_u8();
        let marker = (byte1 >> 7) & 0x01 == 1;
        let payload_type = byte1 & 0x7F;

        let sequence_number = reader.get_u16();
        let timestamp = reader.get_u32();
        let ssrc = reader.get_u32();
        
        // Skip CSRC
        let csrc_count = byte0 & 0x0F;
        reader.advance(csrc_count as usize * 4);

        let payload = reader.copy_to_bytes(reader.remaining()).to_vec();

        Ok(Self {
            version, padding, extension, marker, payload_type,
            sequence_number, timestamp, ssrc,
            payload,
        })
    }
} 