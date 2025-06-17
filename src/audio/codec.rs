use anyhow::Result;
use opus::{Application, Channels, Encoder};

pub struct OpusEncoder {
    encoder: Encoder,
}

impl OpusEncoder {
    pub fn new(sample_rate: u32, channels: Channels) -> Result<Self> {
        let encoder = Encoder::new(sample_rate, channels, Application::Audio)?;
        Ok(Self { encoder })
    }

    pub fn encode(&mut self, input: &[f32], output: &mut [u8]) -> Result<usize> {
        let encoded_len = self.encoder.encode_float(input, output)?;
        Ok(encoded_len)
    }
} 