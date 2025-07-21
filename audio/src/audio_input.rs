use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Sample, SampleFormat};
use rtp_midi_core::event_bus::Event;
use rustfft::num_traits;
use tokio::sync::broadcast;

/// Starts audio capture from the specified device (or default if None).
/// Sends audio buffers (Vec<f32>) to the provided channel sender.
pub fn start_audio_input(
    device_name: Option<&str>,
    tx: broadcast::Sender<Event>,
) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = if let Some(name) = device_name {
        host.input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| anyhow::anyhow!("Audio device not found: {}", name))?
    } else {
        host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio input device"))?
    };
    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let config = config.into();
    let err_fn = |err| eprintln!("Audio input error: {err}");
    let stream = match sample_format {
        SampleFormat::F32 => build_input_stream::<f32>(&device, &config, tx.clone(), err_fn)?,
        SampleFormat::I16 => build_input_stream::<i16>(&device, &config, tx.clone(), err_fn)?,
        SampleFormat::U16 => build_input_stream::<u16>(&device, &config, tx.clone(), err_fn)?,
        _ => {
            log::error!("Unsupported sample format: {sample_format:?}");
            return Err(anyhow::anyhow!(
                "Unsupported sample format: {:?}",
                sample_format
            ));
        }
    };
    Ok(stream)
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: broadcast::Sender<Event>,
    err_fn: fn(cpal::StreamError),
) -> Result<cpal::Stream>
where
    T: Sample + cpal::SizedSample + num_traits::ToPrimitive + Send + 'static,
{
    let _channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut buffer = Vec::with_capacity(data.len());
            for &sample in data {
                buffer.push(num_traits::ToPrimitive::to_f32(&sample).unwrap_or(0.0));
            }
            // Optionally: downmix to mono or keep as is
            let _ = tx.send(Event::AudioDataReady(buffer));
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
