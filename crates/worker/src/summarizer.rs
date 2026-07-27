//! Client for the configured Ideavibes AI (Claude-compatible) summarization service.

use std::{error::Error, fmt, time::Duration};

use policy_shared::config::AiSummarizerConfig;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_SUMMARY_TOKENS: u32 = 600;
const MAX_SUMMARY_ATTEMPTS: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);

/// Sends policy-change prompts to the configured AI service.
#[derive(Clone)]
pub struct AiSummarizer {
    state: SummarizerState,
}

#[derive(Clone)]
enum SummarizerState {
    Disabled,
    Configured {
        client: Client,
        api_key: String,
        endpoint: String,
        model: String,
    },
}

impl AiSummarizer {
    /// Builds a client with bounded request and connection timeouts.
    pub fn from_config(config: &AiSummarizerConfig) -> Result<Self, SummarizerError> {
        Self::from_optional_config(Some(config))
    }

    /// Builds either a configured client or an explicitly disabled integration.
    pub fn from_optional_config(
        config: Option<&AiSummarizerConfig>,
    ) -> Result<Self, SummarizerError> {
        let Some(config) = config else {
            return Ok(Self {
                state: SummarizerState::Disabled,
            });
        };

        let request_timeout = nonzero_timeout(config.request_timeout)?;
        let client = Client::builder()
            .connect_timeout(request_timeout.min(Duration::from_secs(5)))
            .timeout(request_timeout)
            .build()
            .map_err(SummarizerError::Client)?;

        Ok(Self {
            state: SummarizerState::Configured {
                client,
                api_key: config.api_key.clone(),
                endpoint: format!("{}/messages", config.base_url.trim_end_matches('/')),
                model: config.model.clone(),
            },
        })
    }

    /// Indicates whether requests will be sent to an AI service.
    pub fn is_enabled(&self) -> bool {
        matches!(self.state, SummarizerState::Configured { .. })
    }

    /// Requests a concise summary for a policy-change prompt.
    ///
    /// A disabled integration produces no summary. Configured integrations
    /// retry temporary network and service failures before returning an error
    /// to the ingestion hook, which records the change without a summary.
    pub async fn summarize(&self, prompt: &str) -> Result<Option<String>, SummarizerError> {
        let SummarizerState::Configured {
            client,
            api_key,
            endpoint,
            model,
        } = &self.state
        else {
            return Ok(None);
        };

        for attempt in 1..=MAX_SUMMARY_ATTEMPTS {
            match summarize_once(client, api_key, endpoint, model, prompt).await {
                Ok(summary) => return Ok(Some(summary)),
                Err(error) if attempt < MAX_SUMMARY_ATTEMPTS && error.is_retryable() => {
                    let delay = RETRY_DELAY * u32::from(attempt);
                    eprintln!(
                        "AI summarizer attempt {attempt}/{MAX_SUMMARY_ATTEMPTS} failed temporarily: {error}; retrying in {}ms",
                        delay.as_millis()
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("the bounded summary retry loop always returns")
    }
}

async fn summarize_once(
    client: &Client,
    api_key: &str,
    endpoint: &str,
    model: &str,
    prompt: &str,
) -> Result<String, SummarizerError> {
    let response = client
        .post(endpoint)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("x-api-key", api_key)
        .json(&json!({
            "model": model,
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

/// A summarizer client could not complete a request or parse a response.
#[derive(Debug)]
pub enum SummarizerError {
    Client(reqwest::Error),
    InvalidTimeout,
    Request(reqwest::Error),
    Service { status: StatusCode },
    InvalidResponse,
}

impl SummarizerError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Request(error) => {
                error.is_timeout() || error.is_connect() || error.is_request() || error.is_body()
            }
            Self::Service { status } => matches!(
                *status,
                StatusCode::REQUEST_TIMEOUT
                    | StatusCode::TOO_EARLY
                    | StatusCode::TOO_MANY_REQUESTS
                    | StatusCode::INTERNAL_SERVER_ERROR
                    | StatusCode::BAD_GATEWAY
                    | StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::GATEWAY_TIMEOUT
            ),
            Self::Client(_) | Self::InvalidTimeout | Self::InvalidResponse => false,
        }
    }
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
    use std::time::Duration;

    use reqwest::StatusCode;

    use super::{extract_text, nonzero_timeout, AiSummarizer, SummarizerError};

    #[test]
    fn extracts_text_from_claude_response() {
        let result = extract_text(r#"{"content":[{"type":"text","text":" Summary "}]}"#);
        match result {
            Ok(summary) => assert_eq!(summary, "Summary"),
            Err(error) => panic!("valid Claude response should contain text: {error}"),
        }
    }

    #[test]
    fn rejects_empty_timeout_and_missing_text() {
        assert!(nonzero_timeout(Duration::ZERO).is_err());
        assert!(extract_text(r#"{"content":[]}"#).is_err());
    }

    #[test]
    fn retries_only_transient_service_statuses() {
        assert!(SummarizerError::Service {
            status: StatusCode::TOO_MANY_REQUESTS
        }
        .is_retryable());
        assert!(SummarizerError::Service {
            status: StatusCode::SERVICE_UNAVAILABLE
        }
        .is_retryable());
        assert!(!SummarizerError::Service {
            status: StatusCode::BAD_REQUEST
        }
        .is_retryable());
    }

    #[tokio::test]
    async fn disabled_summarizer_skips_requests() {
        let summarizer = match AiSummarizer::from_optional_config(None) {
            Ok(summarizer) => summarizer,
            Err(error) => panic!("disabled summarizer should be valid: {error}"),
        };

        assert!(!summarizer.is_enabled());
        let result = summarizer.summarize("No request should be sent").await;
        assert!(matches!(result, Ok(None)));
    }
}
