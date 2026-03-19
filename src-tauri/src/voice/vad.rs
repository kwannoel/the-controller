use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

/// Voice Activity Detection events.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    SpeechStart,
    SpeechEnd,
}

/// Context size prepended to each chunk (last N samples from previous chunk).
/// Required by Silero VAD ONNX model — without this, probabilities stay near zero.
const CONTEXT_SIZE: usize = 64;
const STATE_SIZE: usize = 256;

pub struct Vad {
    session: Session,
    state: Vec<f32>,
    context: Vec<f32>,
    triggered: bool,
    temp_end: usize,
    current_sample: usize,
    threshold: f32,
    min_silence_samples: usize,
    last_prob: f32,
}

impl Vad {
    pub fn new(model_path: &Path, min_silence_ms: u32) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create ONNX session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("Failed to set threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load Silero VAD model: {e}"))?;

        let min_silence_samples = (min_silence_ms as usize * 16000) / 1000;

        Ok(Self {
            session,
            state: vec![0.0f32; STATE_SIZE],
            context: vec![0.0f32; CONTEXT_SIZE],
            triggered: false,
            temp_end: 0,
            current_sample: 0,
            threshold: 0.5,
            min_silence_samples,
            last_prob: 0.0,
        })
    }

    /// Process a chunk of float32 audio (512 samples at 16kHz).
    /// Returns a VadEvent if a speech boundary was detected.
    pub fn process(&mut self, chunk: &[f32]) -> Result<Option<VadEvent>, String> {
        let chunk_len = chunk.len();
        self.current_sample += chunk_len;

        // Prepend context (last 64 samples from previous chunk) — required by Silero ONNX model
        let mut input_with_context = Vec::with_capacity(CONTEXT_SIZE + chunk_len);
        input_with_context.extend_from_slice(&self.context);
        input_with_context.extend_from_slice(chunk);

        let full_len = input_with_context.len();

        let input_tensor = Tensor::from_array(([1usize, full_len], input_with_context))
            .map_err(|e| format!("Failed to create input tensor: {e}"))?;
        let state_tensor = Tensor::from_array(([2usize, 1, 128], self.state.clone()))
            .map_err(|e| format!("Failed to create state tensor: {e}"))?;
        let sr_tensor = Tensor::from_array(((), vec![16000i64]))
            .map_err(|e| format!("Failed to create sr tensor: {e}"))?;

        let outputs = self
            .session
            .run(ort::inputs! {
                "input" => input_tensor,
                "state" => state_tensor,
                "sr" => sr_tensor,
            })
            .map_err(|e| format!("VAD inference failed: {e}"))?;

        // Extract output probability
        let (_, output_data) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output: {e}"))?;
        let prob = output_data[0];
        self.last_prob = prob;

        // Extract and update state
        let (_, state_data) = outputs["stateN"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract stateN: {e}"))?;
        self.state = state_data.to_vec();

        // Update context: last CONTEXT_SIZE samples from this chunk
        if chunk_len >= CONTEXT_SIZE {
            self.context
                .copy_from_slice(&chunk[chunk_len - CONTEXT_SIZE..]);
        } else {
            let keep = CONTEXT_SIZE - chunk_len;
            self.context.copy_within(chunk_len.., 0);
            self.context[keep..].copy_from_slice(chunk);
        }

        // State machine
        if prob >= self.threshold && !self.triggered {
            self.triggered = true;
            self.temp_end = 0;
            return Ok(Some(VadEvent::SpeechStart));
        }

        if prob < (self.threshold - 0.15) && self.triggered {
            if self.temp_end == 0 {
                self.temp_end = self.current_sample;
            }
            if self.current_sample - self.temp_end >= self.min_silence_samples {
                self.triggered = false;
                self.temp_end = 0;
                return Ok(Some(VadEvent::SpeechEnd));
            }
        } else if self.triggered {
            self.temp_end = 0;
        }

        Ok(None)
    }

    /// Returns the probability from the last `process()` call.
    pub fn last_prob(&self) -> f32 {
        self.last_prob
    }

    /// Reset state for a new conversation turn.
    pub fn reset(&mut self) {
        self.state = vec![0.0f32; STATE_SIZE];
        self.context = vec![0.0f32; CONTEXT_SIZE];
        self.triggered = false;
        self.temp_end = 0;
        self.current_sample = 0;
        self.last_prob = 0.0;
    }
}
