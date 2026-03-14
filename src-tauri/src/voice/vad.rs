use ort::session::Session;
use ort::value::{Tensor, TensorRef};
use std::path::Path;

/// Voice Activity Detection events.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    SpeechStart,
    SpeechEnd,
}

pub struct Vad {
    session: Session,
    // Silero VAD internal state
    h: ndarray::Array2<f32>,
    c: ndarray::Array2<f32>,
    triggered: bool,
    temp_end: usize,
    current_sample: usize,
    threshold: f32,
    min_silence_samples: usize,
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
            h: ndarray::Array2::zeros((2, 64)),
            c: ndarray::Array2::zeros((2, 64)),
            triggered: false,
            temp_end: 0,
            current_sample: 0,
            threshold: 0.5,
            min_silence_samples,
        })
    }

    /// Process a chunk of float32 audio (512 samples at 16kHz).
    /// Returns a VadEvent if a speech boundary was detected.
    pub fn process(&mut self, chunk: &[f32]) -> Result<Option<VadEvent>, String> {
        let chunk_len = chunk.len();
        self.current_sample += chunk_len;

        // Prepare inputs as ort Tensors
        let input_tensor = Tensor::from_array(([1usize, chunk_len], chunk.to_vec()))
            .map_err(|e| format!("Failed to create input tensor: {e}"))?;
        let sr_tensor = Tensor::from_array(((), vec![16000i64]))
            .map_err(|e| format!("Failed to create sr tensor: {e}"))?;
        let h_tensor =
            TensorRef::from_array_view(self.h.view())
                .map_err(|e| format!("Failed to create h tensor: {e}"))?;
        let c_tensor =
            TensorRef::from_array_view(self.c.view())
                .map_err(|e| format!("Failed to create c tensor: {e}"))?;

        let outputs = self
            .session
            .run(ort::inputs! {
                "input" => input_tensor,
                "sr" => sr_tensor,
                "h" => h_tensor,
                "c" => c_tensor
            })
            .map_err(|e| format!("VAD inference failed: {e}"))?;

        // Extract output probability
        let (_, output_data) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract output: {e}"))?;
        let prob = output_data[0];

        // Extract and update hidden states
        let hn_view = outputs["hn"]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract hn: {e}"))?;
        self.h = hn_view.into_owned().into_dimensionality().map_err(|e| format!("Wrong hn shape: {e}"))?;

        let cn_view = outputs["cn"]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract cn: {e}"))?;
        self.c = cn_view.into_owned().into_dimensionality().map_err(|e| format!("Wrong cn shape: {e}"))?;

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

    /// Reset state for a new conversation turn.
    pub fn reset(&mut self) {
        self.h = ndarray::Array2::zeros((2, 64));
        self.c = ndarray::Array2::zeros((2, 64));
        self.triggered = false;
        self.temp_end = 0;
        self.current_sample = 0;
    }
}
