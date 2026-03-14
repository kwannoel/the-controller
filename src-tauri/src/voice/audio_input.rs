use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 16_000;
pub const BLOCK_SIZE: usize = 512; // Required by Silero VAD

pub struct AudioInput {
    stream: Option<Stream>,
    muted: Arc<AtomicBool>,
}

impl AudioInput {
    /// Create and start mic capture. Captures at the device's native rate/format,
    /// downsamples to 16kHz, and sends i16 chunks of BLOCK_SIZE to `sender`.
    pub fn start(sender: Sender<Vec<i16>>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {e}"))?;

        let native_rate = default_config.sample_rate().0;
        let channels = default_config.channels() as usize;

        eprintln!(
            "[voice] Mic: {:?}, {}Hz, {}ch, {:?}",
            device.name().unwrap_or_default(),
            native_rate,
            channels,
            default_config.sample_format()
        );

        let config: cpal::StreamConfig = default_config.clone().into();
        let muted = Arc::new(AtomicBool::new(false));
        let muted_clone = muted.clone();

        // Accumulate resampled samples until we have BLOCK_SIZE
        let downsample_ratio = native_rate as f64 / SAMPLE_RATE as f64;
        let mut resample_buf: Vec<i16> = Vec::with_capacity(BLOCK_SIZE * 2);
        let mut resample_pos: f64 = 0.0;

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    if muted_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    // Mix to mono if needed, then downsample
                    let mono_samples = if channels == 1 {
                        data.to_vec()
                    } else {
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect()
                    };

                    // Downsample from native_rate to 16kHz using linear interpolation
                    for sample in &mono_samples {
                        resample_pos += 1.0;
                        if resample_pos >= downsample_ratio {
                            resample_pos -= downsample_ratio;
                            // Convert f32 [-1.0, 1.0] to i16
                            let s = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            resample_buf.push(s);

                            if resample_buf.len() >= BLOCK_SIZE {
                                let chunk: Vec<i16> = resample_buf.drain(..BLOCK_SIZE).collect();
                                let _ = sender.try_send(chunk);
                            }
                        }
                    }
                },
                |err| {
                    eprintln!("[voice] Audio input error: {err}");
                },
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start input stream: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            muted,
        })
    }

    pub fn mute(&self) {
        self.muted.store(true, Ordering::Relaxed);
    }

    pub fn unmute(&self) {
        self.muted.store(false, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.stream.take();
    }
}
