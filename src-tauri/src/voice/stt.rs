use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperStt {
    ctx: WhisperContext,
}

impl WhisperStt {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("Failed to load Whisper model: {e}"))?;

        Ok(Self { ctx })
    }

    /// Transcribe float32 16kHz audio to text. Returns empty string if no speech detected.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String, String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        params.set_single_segment(true);

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("Failed to create Whisper state: {e}"))?;

        state
            .full(params, audio)
            .map_err(|e| format!("Whisper inference failed: {e}"))?;

        let n_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get segments: {e}"))?;

        let mut result = String::new();
        for i in 0..n_segments {
            let text = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {e}"))?;

            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip common Whisper hallucinations
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }
            if trimmed == "Thank you." || trimmed == "Thanks for watching!" {
                continue;
            }

            result.push_str(trimmed);
            result.push(' ');
        }

        Ok(result.trim().to_string())
    }
}
