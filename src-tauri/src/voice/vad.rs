use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

/// Voice Activity Detection events.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    SpeechStart,
    SpeechEnd,
}

pub struct Vad {
    session: Session,
    // Silero VAD internal state: shape [2, 1, 128]
    state: Vec<f32>,
    triggered: bool,
    temp_end: usize,
    current_sample: usize,
    threshold: f32,
    min_silence_samples: usize,
    last_prob: f32,
}

const STATE_SIZE: usize = 2 * 1 * 128; // [2, 1, 128]

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

        // Prepare inputs matching actual Silero VAD ONNX model:
        //   input: [1, chunk_len] float32
        //   state: [2, 1, 128] float32
        //   sr: scalar int64
        let input_tensor = Tensor::from_array(([1usize, chunk_len], chunk.to_vec()))
            .map_err(|e| format!("Failed to create input tensor: {e}"))?;
        let state_tensor =
            Tensor::from_array(([2usize, 1, 128], self.state.clone()))
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
        self.triggered = false;
        self.temp_end = 0;
        self.current_sample = 0;
        self.last_prob = 0.0;
    }
}
