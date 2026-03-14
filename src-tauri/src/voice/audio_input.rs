use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, Stream};
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
    /// Create and start mic capture. Sends int16 chunks of BLOCK_SIZE to `sender`.
    pub fn start(sender: Sender<Vec<i16>>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(BLOCK_SIZE as u32),
        };

        let muted = Arc::new(AtomicBool::new(false));
        let muted_clone = muted.clone();

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                    if !muted_clone.load(Ordering::Relaxed) {
                        let _ = sender.try_send(data.to_vec());
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
        self.stream.take(); // Dropping the stream stops capture
    }
}
