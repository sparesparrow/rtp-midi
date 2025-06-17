use crate::audio_analysis::compute_fft_magnitudes;

/// Maps FFT magnitudes to LED RGB values.
pub fn map_audio_to_leds(magnitudes: &[f32], led_count: usize) -> Vec<u8> {
    let mut leds = vec![0u8; led_count * 3];
    let band_size = magnitudes.len() / 3;
    let bass = magnitudes.iter().take(band_size).cloned().fold(0.0, f32::max);
    let mid = magnitudes.iter().skip(band_size).take(band_size).cloned().fold(0.0, f32::max);
    let treble = magnitudes.iter().skip(2 * band_size).cloned().fold(0.0, f32::max);
    for i in 0..led_count {
        leds[i * 3] = (bass * 255.0) as u8;   // Red
        leds[i * 3 + 1] = (mid * 255.0) as u8; // Green
        leds[i * 3 + 2] = (treble * 255.0) as u8; // Blue
    }
    leds
}

#[cfg(test)]
mod mapping_tests {
    use super::*;

    #[test]
    fn test_map_audio_to_leds_bass() {
        let mags = vec![1.0, 0.0, 0.0]; // Only bass
        let leds = map_audio_to_leds(&mags, 2);
        assert_eq!(leds, vec![255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_mid() {
        let mags = vec![0.0, 1.0, 0.0]; // Only mid
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 255, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_treble() {
        let mags = vec![0.0, 0.0, 1.0]; // Only treble
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 0, 255]);
    }
} 