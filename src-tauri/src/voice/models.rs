use std::path::PathBuf;

/// Returns the base directory for voice model storage.
pub fn models_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home directory must exist")
        .join(".the-controller")
        .join("voice-models")
}

pub struct ModelPaths {
    pub silero_vad: PathBuf,
    pub whisper: PathBuf,
    pub piper_onnx: PathBuf,
    pub piper_config: PathBuf,
}

impl Default for ModelPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelPaths {
    pub fn new() -> Self {
        let base = models_dir();
        Self {
            silero_vad: base.join("silero_vad.onnx"),
            whisper: base.join("ggml-base.bin"),
            piper_onnx: base.join("en_US-ryan-medium.onnx"),
            piper_config: base.join("en_US-ryan-medium.onnx.json"),
        }
    }

    /// Returns true if all model files exist on disk.
    pub fn all_present(&self) -> bool {
        self.silero_vad.exists()
            && self.whisper.exists()
            && self.piper_onnx.exists()
            && self.piper_config.exists()
    }

    /// Returns a list of (url, destination) pairs for missing models.
    pub fn missing_downloads(&self) -> Vec<(&'static str, &PathBuf)> {
        let mut missing = Vec::new();
        if !self.silero_vad.exists() {
            missing.push((
                "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
                &self.silero_vad,
            ));
        }
        if !self.whisper.exists() {
            missing.push((
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
                &self.whisper,
            ));
        }
        if !self.piper_onnx.exists() {
            missing.push((
                "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx",
                &self.piper_onnx,
            ));
        }
        if !self.piper_config.exists() {
            missing.push((
                "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ryan/medium/en_US-ryan-medium.onnx.json",
                &self.piper_config,
            ));
        }
        missing
    }
}

/// Download all missing models with progress reporting.
/// `on_progress(filename, bytes_downloaded, total_bytes)` is called periodically.
/// `total_bytes` is `None` if the server didn't send a Content-Length header.
pub async fn ensure_models(
    on_progress: impl Fn(&str, u64, Option<u64>),
) -> Result<ModelPaths, String> {
    use futures_util::StreamExt;

    let paths = ModelPaths::new();
    let downloads = paths.missing_downloads();

    if !downloads.is_empty() {
        std::fs::create_dir_all(models_dir())
            .map_err(|e| format!("Failed to create models dir: {e}"))?;
    }

    for (url, dest) in &downloads {
        let filename = dest.file_name().unwrap().to_string_lossy();
        on_progress(&filename, 0, None);

        let response = reqwest::get(*url)
            .await
            .map_err(|e| format!("Failed to download {filename}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Failed to download {filename}: HTTP {}",
                response.status()
            ));
        }

        let total = response.content_length();
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut body = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Failed to read {filename}: {e}"))?;
            downloaded += chunk.len() as u64;
            body.extend_from_slice(&chunk);
            on_progress(&filename, downloaded, total);
        }

        std::fs::write(dest, &body).map_err(|e| format!("Failed to write {filename}: {e}"))?;
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_point_to_correct_directory() {
        let paths = ModelPaths::new();
        let base = models_dir();
        assert!(paths.silero_vad.starts_with(&base));
        assert!(paths.whisper.starts_with(&base));
        assert!(paths.piper_onnx.starts_with(&base));
        assert!(paths.piper_config.starts_with(&base));
    }

    #[test]
    fn missing_downloads_returns_all_when_none_exist() {
        let paths = ModelPaths {
            silero_vad: PathBuf::from("/nonexistent/vad.onnx"),
            whisper: PathBuf::from("/nonexistent/whisper.bin"),
            piper_onnx: PathBuf::from("/nonexistent/piper.onnx"),
            piper_config: PathBuf::from("/nonexistent/piper.json"),
        };
        assert_eq!(paths.missing_downloads().len(), 4);
        assert!(!paths.all_present());
    }
}
