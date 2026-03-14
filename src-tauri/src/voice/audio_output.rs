use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleRate;
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

    /// Play int16 audio at the given sample rate. Blocks until playback completes.
    pub fn play_i16(&self, samples: &[i16], sample_rate: u32) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let data = Arc::new(samples.to_vec());
        let position = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));

        let data_clone = data.clone();
        let pos_clone = position.clone();
        let done_clone = done.clone();

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |output: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                    let pos = pos_clone.load(Ordering::Relaxed);
                    let remaining = data_clone.len() - pos;
                    let to_write = remaining.min(output.len());

                    output[..to_write].copy_from_slice(&data_clone[pos..pos + to_write]);
                    for sample in output[to_write..].iter_mut() {
                        *sample = 0;
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

        stream.play().map_err(|e| format!("Failed to start output: {e}"))?;

        while !done.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(())
    }
}
