use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct AudioOutput {
    device: cpal::Device,
}

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;
        Ok(Self { device })
    }

    /// Play int16 mono audio at the given sample rate. Blocks until playback completes.
    /// Resamples to device native rate and converts to f32 stereo for macOS compatibility.
    pub fn play_i16(&self, samples: &[i16], sample_rate: u32) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }

        let default_config = self
            .device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {e}"))?;

        let native_rate = default_config.sample_rate().0;
        let channels = default_config.channels() as usize;

        // Upsample from source rate to native rate, convert i16→f32, mono→stereo
        let ratio = native_rate as f64 / sample_rate as f64;
        let output_len = (samples.len() as f64 * ratio) as usize;
        let mut resampled = Vec::with_capacity(output_len * channels);

        for i in 0..output_len {
            let src_pos = i as f64 / ratio;
            let idx = src_pos as usize;
            let frac = (src_pos - idx as f64) as f32;

            let s0 = samples[idx.min(samples.len() - 1)] as f32 / 32768.0;
            let s1 = samples[(idx + 1).min(samples.len() - 1)] as f32 / 32768.0;
            let sample = s0 + (s1 - s0) * frac;

            for _ in 0..channels {
                resampled.push(sample);
            }
        }

        let config: cpal::StreamConfig = default_config.into();
        let data = Arc::new(resampled);
        let position = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));

        let data_clone = data.clone();
        let pos_clone = position.clone();
        let done_clone = done.clone();

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |output: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let pos = pos_clone.load(Ordering::Relaxed);
                    let remaining = data_clone.len() - pos;
                    let to_write = remaining.min(output.len());

                    output[..to_write].copy_from_slice(&data_clone[pos..pos + to_write]);
                    for sample in output[to_write..].iter_mut() {
                        *sample = 0.0;
                    }

                    pos_clone.store(pos + to_write, Ordering::Relaxed);
                    if pos + to_write >= data_clone.len() {
                        done_clone.store(true, Ordering::Relaxed);
                    }
                },
                |err| {
                    eprintln!("[voice] Audio output error: {err}");
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start output: {e}"))?;

        while !done.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(())
    }
}
