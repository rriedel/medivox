//! HTTP-Client zur Engine (.NET-Pendant: EngineClient.cs).
//!
//! POST /transcribe erwartet rohe float32-PCM-Samples (16 kHz mono) als Body und
//! antwortet mit dem erkannten Text als Plaintext.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::config;

pub struct EngineClient {
    agent: ureq::Agent,
    url: String,
}

impl EngineClient {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(config::REQUEST_TIMEOUT_SECS))
            .build();
        let url = format!(
            "http://{}:{}/transcribe",
            config::engine_host(),
            config::engine_port()
        );
        Self { agent, url }
    }

    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        // Little-Endian float32, wie es numpy auf x86 per frombuffer() erwartet.
        let mut bytes = Vec::with_capacity(audio.len() * 4);
        for sample in audio {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        let response = self
            .agent
            .post(&self.url)
            .set("Content-Type", "application/octet-stream")
            .send_bytes(&bytes)
            .context("Anfrage an die Engine fehlgeschlagen")?;

        response
            .into_string()
            .context("Antwort der Engine konnte nicht gelesen werden")
    }
}

impl Default for EngineClient {
    fn default() -> Self {
        Self::new()
    }
}
