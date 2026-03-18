use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioOutput;

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        // Verify an output device exists at construction time
        let host = cpal::default_host();
        host.default_output_device()
            .ok_or("No output device available")?;
        Ok(Self)
    }

    /// Get a fresh device handle. macOS CoreAudio invalidates cached handles
    /// between audio sessions, so we re-query each time.
    fn device(&self) -> Result<cpal::Device, String> {
        let host = cpal::default_host();
        host.default_output_device()
            .ok_or("No output device available".to_string())
    }

    /// Play int16 mono audio at the given sample rate. Blocks until playback completes.
    /// Resamples to device native rate and converts to f32 stereo for macOS compatibility.
    pub fn play_i16(&self, samples: &[i16], sample_rate: u32) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            sample_count = samples.len(),
            sample_rate,
            "starting blocking audio playback"
        );

        let device = self.device()?;
        let default_config = device
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

        let stream = device
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
                    tracing::error!("audio output error: {err}");
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

    /// Start a streaming audio output. Audio can be pushed incrementally via the returned handle.
    /// The cpal stream outputs silence when the buffer is empty (between chunks).
    pub fn start_streaming(&self, source_sample_rate: u32) -> Result<StreamingPlayback, String> {
        tracing::debug!(source_sample_rate, "starting streaming audio playback");
        let device = self.device()?;
        let default_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {e}"))?;

        let native_rate = default_config.sample_rate().0;
        let channels = default_config.channels() as usize;
        let config: cpal::StreamConfig = default_config.into();

        let buffer: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let done_writing = Arc::new(AtomicBool::new(false));
        let done_playing = Arc::new(AtomicBool::new(false));

        let buf_cb = buffer.clone();
        let dw_cb = done_writing.clone();
        let dp_cb = done_playing.clone();

        let stream = device
            .build_output_stream(
                &config,
                move |output: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let mut buf = buf_cb.lock().unwrap_or_else(|e| e.into_inner());
                    for sample in output.iter_mut() {
                        if let Some(s) = buf.pop_front() {
                            *sample = s;
                        } else {
                            *sample = 0.0;
                        }
                    }
                    if dw_cb.load(Ordering::Relaxed) && buf.is_empty() {
                        dp_cb.store(true, Ordering::Relaxed);
                    }
                },
                |err| tracing::error!("streaming audio error: {err}"),
                None,
            )
            .map_err(|e| format!("Failed to build streaming output: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start streaming output: {e}"))?;

        Ok(StreamingPlayback {
            buffer,
            done_writing,
            done_playing,
            _stream: stream,
            native_rate,
            channels,
            source_rate: source_sample_rate,
        })
    }
}

/// Handle for streaming audio playback. Push audio chunks incrementally;
/// the cpal stream plays them continuously without gaps.
pub struct StreamingPlayback {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    done_writing: Arc<AtomicBool>,
    done_playing: Arc<AtomicBool>,
    _stream: cpal::Stream,
    native_rate: u32,
    channels: usize,
    source_rate: u32,
}

impl StreamingPlayback {
    /// Push a chunk of i16 mono audio. Resamples and enqueues for playback.
    pub fn push_samples(&self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        let ratio = self.native_rate as f64 / self.source_rate as f64;
        let output_len = (samples.len() as f64 * ratio) as usize;
        let mut resampled = Vec::with_capacity(output_len * self.channels);

        for i in 0..output_len {
            let src_pos = i as f64 / ratio;
            let idx = src_pos as usize;
            let frac = (src_pos - idx as f64) as f32;

            let s0 = samples[idx.min(samples.len() - 1)] as f32 / 32768.0;
            let s1 = samples[(idx + 1).min(samples.len() - 1)] as f32 / 32768.0;
            let sample = s0 + (s1 - s0) * frac;

            for _ in 0..self.channels {
                resampled.push(sample);
            }
        }

        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.extend(resampled);
    }

    /// Signal that no more audio will be pushed and wait for playback to drain.
    pub fn finish(self) {
        tracing::debug!("streaming playback finishing, waiting for drain");
        self.done_writing.store(true, Ordering::Relaxed);
        while !self.done_playing.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    /// Signal that no more audio will be pushed, without waiting for drain.
    pub fn seal(&self) {
        self.done_writing.store(true, Ordering::Relaxed);
    }

    /// Cancel playback immediately — clear the buffer and signal done.
    pub fn cancel(self) {
        tracing::debug!("streaming playback cancelled");
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.clear();
        }
        self.done_writing.store(true, Ordering::Relaxed);
        // Brief wait for cpal callback to see the empty buffer
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    /// Check if all pushed audio has finished playing.
    pub fn is_done(&self) -> bool {
        self.done_playing.load(Ordering::Relaxed)
    }
}
