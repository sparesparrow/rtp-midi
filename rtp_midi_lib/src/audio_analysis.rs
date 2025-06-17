use rustfft::{FftPlanner, num_complex::Complex};

/// Performs FFT on the input buffer and returns normalized magnitudes.
pub fn compute_fft_magnitudes(input: &[f32], prev: &mut Vec<f32>, smoothing: f32) -> Vec<f32> {
    let len = input.len().next_power_of_two();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(len);
    let mut buffer: Vec<Complex<f32>> = input.iter().map(|&x| Complex{ re: x, im: 0.0 }).collect();
    buffer.resize(len, Complex{ re: 0.0, im: 0.0 });
    fft.process(&mut buffer);
    let mut mags: Vec<f32> = buffer.iter().map(|c| c.norm()).collect();
    // Normalize
    let max = mags.iter().cloned().fold(0.0_f32, f32::max).max(1e-6);
    for m in mags.iter_mut() { *m /= max; }
    // Smoothing (simple moving average with previous frame)
    if prev.len() == mags.len() {
        for (m, p) in mags.iter_mut().zip(prev.iter()) {
            *m = smoothing * *p + (1.0 - smoothing) * *m;
        }
    }
    *prev = mags.clone();
    mags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_sine_wave() {
        // Generate a sine wave at 1/16th of the sample rate
        let n = 64;
        let freq_bin = 4;
        let mut input = vec![0.0f32; n];
        for i in 0..n {
            input[i] = (2.0 * PI * freq_bin as f32 * i as f32 / n as f32).sin();
        }
        let mut prev = vec![];
        let mags = compute_fft_magnitudes(&input, &mut prev, 0.0);
        // The magnitude should peak at bin 4 or n-4 (due to symmetry)
        let max_idx = mags.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert!(max_idx == freq_bin || max_idx == n - freq_bin, "Peak at wrong bin: {}", max_idx);
        // The peak should be much higher than the average
        let peak = mags[max_idx];
        let avg = mags.iter().sum::<f32>() / mags.len() as f32;
        assert!(peak > 3.0 * avg, "Peak not prominent enough");
    }

    #[test]
    fn test_fft_smoothing() {
        let n = 8;
        let input1 = vec![1.0; n];
        let input2 = vec![0.0; n];
        let mut prev = vec![0.0; n];
        let mags1 = compute_fft_magnitudes(&input1, &mut prev, 0.5);
        let mags2 = compute_fft_magnitudes(&input2, &mut prev, 0.5);
        // After smoothing, mags2 should be halfway between mags1 and 0
        for (m, m1) in mags2.iter().zip(mags1.iter()) {
            assert!((*m - m1 * 0.5).abs() < 1e-3, "Smoothing failed");
        }
    }
} 