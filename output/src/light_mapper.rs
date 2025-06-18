/// Maps FFT magnitudes to LED RGB values.
pub fn map_audio_to_leds(magnitudes: &[f32], led_count: usize) -> Vec<u8> {
    let mut leds = Vec::with_capacity(led_count * 3);
    for i in 0..led_count {
        let hue = i as f32 / led_count as f32;
        let magnitude_index = (i as f32 / led_count as f32 * magnitudes.len() as f32) as usize;
        let brightness = magnitudes.get(magnitude_index).cloned().unwrap_or(0.0).sqrt() * 2.0;
        let (r, g, b) = hsv_to_rgb(hue, 1.0, brightness.min(1.0));
        leds.push(r);
        leds.push(g);
        leds.push(b);
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
    fn test_map_audio_to_leds_bass() {
        let mags = vec![1.0, 0.0, 0.0]; // Only bass
        let leds = map_audio_to_leds(&mags, 2);
        assert_eq!(leds, vec![255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_mid() {
        let mags = vec![0.0, 1.0, 0.0]; // Only mid
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 0, 0]);
    }

    #[test]
    fn test_map_audio_to_leds_treble() {
        let mags = vec![0.0, 0.0, 1.0]; // Only treble
        let leds = map_audio_to_leds(&mags, 1);
        assert_eq!(leds, vec![0, 0, 0]);
    }
} 