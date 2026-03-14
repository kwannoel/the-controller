use std::collections::VecDeque;

/// Adaptive gain control. Tracks rolling RMS and applies gain to maintain
/// a target loudness level. Converts int16 input to normalized float32.
pub struct AutoGain {
    target_rms: f32,
    max_gain: f32,
    history: VecDeque<f32>,
    window_size: usize,
}

impl AutoGain {
    pub fn new() -> Self {
        Self {
            target_rms: 0.1,
            max_gain: 100.0,
            history: VecDeque::new(),
            window_size: 31,
        }
    }

    /// Process a chunk of int16 audio samples. Returns float32 in [-1.0, 1.0].
    pub fn apply(&mut self, samples: &[i16]) -> Vec<f32> {
        // Convert to float32
        let float_samples: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();

        // Calculate RMS
        let rms = {
            let sum_sq: f32 = float_samples.iter().map(|&s| s * s).sum();
            (sum_sq / float_samples.len() as f32).sqrt()
        };

        // Skip near-silence
        if rms < 1e-6 {
            return float_samples;
        }

        // Update history
        self.history.push_back(rms);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // Calculate average RMS
        let avg_rms: f32 = self.history.iter().sum::<f32>() / self.history.len() as f32;
        if avg_rms < 1e-6 {
            return float_samples;
        }

        // Apply gain
        let gain = (self.target_rms / avg_rms).min(self.max_gain);
        float_samples
            .iter()
            .map(|&s| (s * gain).clamp(-1.0, 1.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_passes_through_unchanged() {
        let mut gain = AutoGain::new();
        let silence = vec![0i16; 512];
        let result = gain.apply(&silence);
        assert!(result.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn output_is_clamped_to_unit_range() {
        let mut gain = AutoGain::new();
        // Very quiet signal — gain will be high
        let quiet: Vec<i16> = (0..512).map(|i| (i % 3) as i16).collect();
        for _ in 0..50 {
            let result = gain.apply(&quiet);
            assert!(result.iter().all(|&s| s >= -1.0 && s <= 1.0));
        }
    }

    #[test]
    fn converts_int16_to_float32() {
        let mut gain = AutoGain::new();
        let input = vec![16384i16]; // 0.5 in float
        let result = gain.apply(&input);
        assert!(!result.is_empty());
        assert!(result[0].is_finite());
        assert!(result[0] >= -1.0 && result[0] <= 1.0);
    }
}
