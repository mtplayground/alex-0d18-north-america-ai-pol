//! Client for the configured Ideavibes AI (Claude-compatible) summarization service.

use std::{error::Error, fmt, time::Duration};

use policy_shared::config::AiSummarizerConfig;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_SUMMARY_TOKENS: u32 = 600;

/// Sends policy-change prompts to the configured AI service.
#[derive(Clone)]
pub struct AiSummarizer {
    client: Client,
    api_key: String,
    endpoint: String,
    model: String,
}

impl AiSummarizer {
    /// Builds a client with bounded request and connection timeouts.
    pub fn from_config(config: &AiSummarizerConfig) -> Result<Self, SummarizerError> {
        let request_timeout = nonzero_timeout(config.request_timeout)?;
        let client = Client::builder()
            .connect_timeout(request_timeout.min(Duration::from_secs(5)))
            .timeout(request_timeout)
            .build()
            .map_err(SummarizerError::Client)?;

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            endpoint: format!("{}/messages", config.base_url.trim_end_matches('/')),
            model: config.model.clone(),
        })
    }

    /// Requests a concise summary for a policy-change prompt.
    pub async fn summarize(&self, prompt: &str) -> Result<String, SummarizerError> {
        let response = self
            .client
            .post(&self.endpoint)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", &self.api_key)
            .json(&json!({
                "model": self.model,
                "max_tokens": MAX_SUMMARY_TOKENS,
                "messages": [{ "role": "user", "content": prompt }],
            }))
            .send()
            .await
            .map_err(SummarizerError::Request)?;

        let status = response.status();
        let body = response.text().await.map_err(SummarizerError::Request)?;
        if !status.is_success() {
            return Err(SummarizerError::Service { status });
        }

        extract_text(&body)
    }
}

/// A summarizer client could not complete a request or parse a response.
#[derive(Debug)]
pub enum SummarizerError {
    Client(reqwest::Error),
    InvalidTimeout,
    Request(reqwest::Error),
    Service { status: StatusCode },
    InvalidResponse,
}

impl fmt::Display for SummarizerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(error) => write!(formatter, "could not configure AI summarizer client: {error}"),
            Self::InvalidTimeout => formatter.write_str("AI summarizer timeout must be greater than zero"),
            Self::Request(error) => write!(formatter, "AI summarizer request failed: {error}"),
            Self::Service { status } => write!(formatter, "AI summarizer service returned {status}"),
            Self::InvalidResponse => formatter.write_str("AI summarizer returned an invalid response"),
        }
    }
}

impl Error for SummarizerError {}

fn nonzero_timeout(timeout: Duration) -> Result<Duration, SummarizerError> {
    if timeout.is_zero() {
        Err(SummarizerError::InvalidTimeout)
    } else {
        Ok(timeout)
    }
}

fn extract_text(body: &str) -> Result<String, SummarizerError> {
    let response: Value = serde_json::from_str(body).map_err(|_| SummarizerError::InvalidResponse)?;
    let text = response
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks.iter().find_map(|block| {
                (block.get("type").and_then(Value::as_str) == Some("text"))
                    .then(|| block.get("text").and_then(Value::as_str))
                    .flatten()
            })
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or(SummarizerError::InvalidResponse)?;

    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{extract_text, nonzero_timeout};
    use std::time::Duration;

    #[test]
    fn extracts_text_from_claude_response() {
        assert_eq!(
            extract_text(r#"{"content":[{"type":"text","text":" Summary "}]}"#).unwrap(),
            "Summary"
        );
    }

    #[test]
    fn rejects_empty_timeout_and_missing_text() {
        assert!(nonzero_timeout(Duration::ZERO).is_err());
        assert!(extract_text(r#"{"content":[]}"#).is_err());
    }
}
