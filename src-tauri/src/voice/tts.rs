use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct PiperTts {
    session: Session,
    sample_rate: u32,
    phoneme_id_map: HashMap<char, Vec<i64>>,
}

impl PiperTts {
    pub fn new(model_path: &Path, config_path: &Path) -> Result<Self, String> {
        let mut builder = Session::builder()
            .map_err(|e| format!("Failed to create TTS session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("Failed to set TTS threads: {e}"))?;

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load Piper model: {e}"))?;

        // Load phoneme ID map from config JSON
        let config_str = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read Piper config: {e}"))?;
        let config: serde_json::Value = serde_json::from_str(&config_str)
            .map_err(|e| format!("Failed to parse Piper config: {e}"))?;

        let mut phoneme_id_map = HashMap::new();
        if let Some(map) = config.get("phoneme_id_map").and_then(|m| m.as_object()) {
            for (key, value) in map {
                if let Some(ids) = value.as_array() {
                    let id_vec: Vec<i64> = ids.iter().filter_map(|v| v.as_i64()).collect();
                    // Key is a single character
                    if let Some(ch) = key.chars().next() {
                        phoneme_id_map.insert(ch, id_vec);
                    }
                }
            }
        }

        if phoneme_id_map.is_empty() {
            return Err("Piper config missing phoneme_id_map".to_string());
        }

        // Read sample rate from config, default to 22050
        let sample_rate = config
            .get("audio")
            .and_then(|a| a.get("sample_rate"))
            .and_then(|s| s.as_u64())
            .unwrap_or(22050) as u32;

        Ok(Self {
            session,
            sample_rate,
            phoneme_id_map,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Convert text to phoneme IDs using espeak-ng + the Piper phoneme_id_map.
    fn phonemize(&self, text: &str) -> Result<Vec<i64>, String> {
        let output = Command::new("espeak-ng")
            .args(["--ipa", "-q", "-v", "en-us", text])
            .output()
            .map_err(|e| {
                format!("Failed to run espeak-ng (is it installed? brew install espeak-ng): {e}")
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("espeak-ng failed: {stderr}"));
        }

        let phonemes = String::from_utf8_lossy(&output.stdout);

        // Piper convention: BOS = '^', EOS = '$', pad = '_'
        let mut ids: Vec<i64> = Vec::new();

        // Add BOS
        if let Some(bos_ids) = self.phoneme_id_map.get(&'^') {
            ids.extend(bos_ids);
        }

        for ch in phonemes.trim().chars() {
            if let Some(char_ids) = self.phoneme_id_map.get(&ch) {
                ids.extend(char_ids);
            }
            // Insert pad (id 0) between each phoneme for better synthesis
            if let Some(pad_ids) = self.phoneme_id_map.get(&'_') {
                ids.extend(pad_ids);
            }
        }

        // Add EOS
        if let Some(eos_ids) = self.phoneme_id_map.get(&'$') {
            ids.extend(eos_ids);
        }

        Ok(ids)
    }

    /// Synthesize text to int16 audio samples.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<i16>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let phoneme_ids = self.phonemize(text)?;
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
