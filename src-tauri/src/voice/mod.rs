pub mod audio_input;
pub mod audio_output;
pub mod gain;
pub mod llm;
pub mod models;
pub mod stt;
pub mod tts;
pub mod vad;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceState {
    Listening,
    Thinking,
    Speaking,
    Downloading,
}
