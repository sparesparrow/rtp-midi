use std::cmp::min;

/// LED mapping presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingPreset {
    Spectrum,
    VuMeter,
}

/// Main entry: map magnitudes to LED RGB values using the selected preset
pub fn map_leds_with_preset(
    magnitudes: &[f32],
    led_count: usize,
    preset: MappingPreset,
) -> Vec<u8> {
    match preset {
        MappingPreset::Spectrum => map_audio_to_leds_spectrum(magnitudes, led_count),
        MappingPreset::VuMeter => map_audio_to_leds_vumeter(magnitudes, led_count),
    }
}

/// Spectrum: original hue-based mapping
pub fn map_audio_to_leds_spectrum(magnitudes: &[f32], led_count: usize) -> Vec<u8> {
    let mut leds = Vec::with_capacity(led_count * 3);
    for i in 0..led_count {
        let hue = i as f32 / led_count as f32;
        let magnitude_index = (i as f32 / led_count as f32 * magnitudes.len() as f32) as usize;
        let brightness = magnitudes
            .get(magnitude_index)
            .cloned()
            .unwrap_or(0.0)
            .sqrt()
            * 2.0;
        let (r, g, b) = hsv_to_rgb(hue, 1.0, brightness.min(1.0));
        leds.push(r);
        leds.push(g);
        leds.push(b);
    }
    leds
}

/// VuMeter: fill LEDs from start based on average magnitude
pub fn map_audio_to_leds_vumeter(magnitudes: &[f32], led_count: usize) -> Vec<u8> {
    let avg = if magnitudes.is_empty() {
        0.0
    } else {
        magnitudes.iter().copied().sum::<f32>() / magnitudes.len() as f32
    };
    let lit_leds = min(led_count, (avg * led_count as f32).round() as usize);
    let mut leds = Vec::with_capacity(led_count * 3);
    for i in 0..led_count {
        if i < lit_leds {
            leds.push(0);
            leds.push(255);
            leds.push(0); // Green for active
        } else {
            leds.push(10);
            leds.push(10);
            leds.push(10); // Dim for inactive
        }
    }
    leds
}

// Temporary placeholder for color conversion, ideally in a separate util module
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0) as u32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[cfg(test)]
mod mapping_tests {
    use super::*;

    #[test]
    fn test_map_audio_to_leds_spectrum_bass() {
        let mags = vec![1.0, 0.0, 0.0]; // Only bass
        let leds = map_audio_to_leds_spectrum(&mags, 2);
        assert_eq!(leds, vec![255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_vumeter() {
        let mags = vec![0.5, 0.5, 0.5];
        let leds = map_audio_to_leds_vumeter(&mags, 4);
        // Should light up 2 LEDs (rounded)
        assert_eq!(leds[0..6], [0, 255, 0, 0, 255, 0]);
        assert_eq!(leds[6..], [10, 10, 10, 10, 10, 10]);
    }

    #[test]
    fn test_map_leds_with_preset_spectrum() {
        let mags = vec![1.0, 0.0, 0.0];
        let leds = map_leds_with_preset(&mags, 2, MappingPreset::Spectrum);
        assert_eq!(leds, vec![255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_map_leds_with_preset_vumeter() {
        let mags = vec![1.0, 1.0, 1.0];
        let leds = map_leds_with_preset(&mags, 3, MappingPreset::VuMeter);
        assert_eq!(leds, vec![0, 255, 0, 0, 255, 0, 0, 255, 0]);
    }
}
