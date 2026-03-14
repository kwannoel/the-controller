use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;
use std::process::Command;

pub struct PiperTts {
    session: Session,
    sample_rate: u32,
}

impl PiperTts {
    pub fn new(model_path: &Path, _config_path: &Path) -> Result<Self, String> {
        let mut builder = Session::builder()
            .map_err(|e| format!("Failed to create TTS session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("Failed to set TTS threads: {e}"))?;

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load Piper model: {e}"))?;

        Ok(Self {
            session,
            sample_rate: 22050,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Convert text to phoneme IDs using espeak-ng.
    fn phonemize(text: &str) -> Result<Vec<i64>, String> {
        let output = Command::new("espeak-ng")
            .args(["--ipa", "-q", "--sep= ", "-v", "en-us", text])
            .output()
            .map_err(|e| {
                format!(
                    "Failed to run espeak-ng (is it installed? brew install espeak-ng): {e}"
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("espeak-ng failed: {stderr}"));
        }

        let phonemes = String::from_utf8_lossy(&output.stdout);

        let mut ids: Vec<i64> = vec![0]; // BOS pad
        for ch in phonemes.trim().chars() {
            if ch == ' ' {
                ids.push(0);
            } else {
                ids.push(ch as i64);
            }
        }
        ids.push(0); // EOS pad

        Ok(ids)
    }

    /// Synthesize text to int16 audio samples.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<i16>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let phoneme_ids = Self::phonemize(text)?;
        let id_count = phoneme_ids.len();

        let input = Tensor::from_array(([1, id_count], phoneme_ids))
            .map_err(|e| format!("Failed to create phoneme tensor: {e}"))?;
        let input_lengths = Tensor::from_array(([1usize], vec![id_count as i64]))
            .map_err(|e| format!("Failed to create input_lengths tensor: {e}"))?;
        let scales = Tensor::from_array(([3usize], vec![0.667f32, 1.0, 0.8]))
            .map_err(|e| format!("Failed to create scales tensor: {e}"))?;

        let outputs = self
            .session
            .run(ort::inputs! {
                "input" => input,
                "input_lengths" => input_lengths,
                "scales" => scales,
            })
            .map_err(|e| format!("TTS inference failed: {e}"))?;

        let (_shape, audio_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract TTS output: {e}"))?;

        let samples: Vec<i16> = audio_data
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        Ok(samples)
    }

    /// Synthesize text sentence by sentence, yielding audio chunks.
    pub fn synthesize_streaming(&mut self, text: &str) -> Vec<Result<Vec<i16>, String>> {
        let sentences = split_sentences(text);
        sentences
            .into_iter()
            .map(|sentence| self.synthesize(&sentence))
            .collect()
    }
}

/// Split text into sentences at natural boundaries.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sentences_basic() {
        let result = split_sentences("Hello there. How are you? I'm fine!");
        assert_eq!(result, vec!["Hello there.", "How are you?", "I'm fine!"]);
    }

    #[test]
    fn split_sentences_no_punctuation() {
        let result = split_sentences("Hello there how are you");
        assert_eq!(result, vec!["Hello there how are you"]);
    }

    #[test]
    fn split_sentences_empty() {
        let result = split_sentences("");
        assert!(result.is_empty());
    }
}
