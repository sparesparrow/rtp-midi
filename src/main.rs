use crossbeam_channel::{bounded, Sender, Receiver};
use std::thread;
use std::time::Duration;
use log::{info, error};
use cpal::traits::StreamTrait;
use rtp_midi::{Config, start_audio_input, compute_fft_magnitudes, map_audio_to_leds, create_ddp_sender, send_ddp_frame};

fn main() {
    // Initialize logging
    env_logger::init();
    // Load config
    let config = match Config::load_from_file("config.toml") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };
    info!("Loaded config: {:?}", config);

    // Channel: audio_input -> audio_analysis
    let (audio_tx, audio_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = bounded(8);
    // Channel: audio_analysis -> light_mapper
    let (fft_tx, fft_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = bounded(8);
    // Channel: light_mapper -> ddp_output
    let (led_tx, led_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = bounded(8);

    // Audio input thread
    let audio_device = config.audio_device.clone();
    let audio_input_handle = thread::spawn(move || {
        match start_audio_input(audio_device.as_deref(), audio_tx) {
            Ok(stream) => {
                info!("Audio input started");
                stream.play().expect("Failed to start audio stream");
                // Let the stream run for a while (test run)
                thread::sleep(Duration::from_secs(3));
                info!("Audio input test run complete");
            }
            Err(e) => error!("Audio input error: {}", e),
        }
    });

    // Audio analysis thread
    let fft_tx2 = fft_tx.clone();
    let analysis_handle = thread::spawn(move || {
        let mut prev = Vec::new();
        let smoothing = 0.7;
        for _ in 0..5 { // Test: process 5 frames
            if let Ok(buffer) = audio_rx.recv_timeout(Duration::from_secs(1)) {
                let mags = compute_fft_magnitudes(&buffer, &mut prev, smoothing);
                fft_tx2.send(mags).expect("Failed to send FFT mags");
            } else {
                error!("Timeout waiting for audio buffer");
            }
        }
        info!("Audio analysis test run complete");
    });

    // Light mapping thread
    let led_tx2 = led_tx.clone();
    let led_count = config.led_count;
    let mapping_handle = thread::spawn(move || {
        for _ in 0..5 {
            if let Ok(mags) = fft_rx.recv_timeout(Duration::from_secs(1)) {
                let leds = map_audio_to_leds(&mags, led_count);
                led_tx2.send(leds).expect("Failed to send LED data");
            } else {
                error!("Timeout waiting for FFT mags");
            }
        }
        info!("Light mapping test run complete");
    });

    // DDP output thread (now actually sends to WLED)
    let wled_ip = config.wled_ip.clone();
    let ddp_port = config.ddp_port.unwrap_or(4048); // Default to 4048 if not set
    let led_count = config.led_count;
    let color_format = config.color_format.clone().unwrap_or_else(|| "RGB".to_string()); // Default to RGB if not set
    let ddp_handle = thread::spawn(move || {
        // Only RGB is supported by ddp-rs 0.3
        let rgbw = color_format.eq_ignore_ascii_case("RGBW");
        let mut sender = match create_ddp_sender(&wled_ip, ddp_port, led_count, rgbw) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create DDP sender: {}", e);
                return;
            }
        };
        for _ in 0..5 {
            if let Ok(leds) = led_rx.recv_timeout(Duration::from_secs(1)) {
                match send_ddp_frame(&mut sender, &leds) {
                    Ok(_) => info!("Sent {} bytes to DDP", leds.len()),
                    Err(e) => error!("Failed to send DDP frame: {}", e),
                }
            } else {
                error!("Timeout waiting for LED data");
            }
        }
        info!("DDP output test run complete");
    });

    // Wait for all threads to finish
    let _ = audio_input_handle.join();
    let _ = analysis_handle.join();
    let _ = mapping_handle.join();
    let _ = ddp_handle.join();

    info!("Test event loop complete. Exiting.");
}