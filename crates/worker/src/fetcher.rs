//! HTTP retrieval for configured government sources.

use std::{error::Error, time::Duration};

use policy_shared::Source;
use reqwest::{header::CONTENT_TYPE, Client, StatusCode, Url};

const MAX_FETCH_ATTEMPTS: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);

/// Raw document fetched from a configured source.
pub struct FetchedPayload {
    /// Fully resolved URL used for the request.
    pub url: Url,
    /// Response media type, when supplied by the source.
    pub content_type: String,
    /// Unmodified response bytes.
    pub body: Vec<u8>,
}

/// HTTP client with bounded requests for source ingestion.
pub struct SourceFetcher {
    client: Client,
}

impl SourceFetcher {
    /// Creates a client with a timeout appropriate for scheduled ingestion.
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder().timeout(Duration::from_secs(30)).build()?,
        })
    }

    /// Fetches the configured start document, retrying temporary source failures.
    pub async fn fetch(
        &self,
        source: &Source,
    ) -> Result<FetchedPayload, Box<dyn Error + Send + Sync>> {
        let url = source_fetch_url(source)?;

        for attempt in 1..=MAX_FETCH_ATTEMPTS {
            match self.fetch_once(url.clone()).await {
                Ok(payload) => return Ok(payload),
                Err(error) if attempt < MAX_FETCH_ATTEMPTS && error.is_retryable() => {
                    let delay = RETRY_DELAY * u32::from(attempt);
                    eprintln!(
                        "fetch attempt {attempt}/{MAX_FETCH_ATTEMPTS} for {url} failed temporarily: {}; retrying in {}ms",
                        error,
                        delay.as_millis()
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(Box::new(error)),
            }
        }

        unreachable!("the bounded fetch retry loop always returns")
    }

    async fn fetch_once(&self, url: Url) -> Result<FetchedPayload, FetchError> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(FetchError::Request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::Status(status));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let body = response
            .bytes()
            .await
            .map_err(FetchError::Request)?
            .to_vec();

        Ok(FetchedPayload {
            url,
            content_type,
            body,
        })
    }
}

#[derive(Debug)]
enum FetchError {
    Request(reqwest::Error),
    Status(StatusCode),
}

impl FetchError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Request(error) => error.is_timeout() || error.is_connect() || error.is_request(),
            Self::Status(status) => is_retryable_status(*status),
        }
    }
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => write!(formatter, "request failed: {error}"),
            Self::Status(status) => write!(formatter, "source returned HTTP {status}"),
        }
    }
}

impl Error for FetchError {}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn source_fetch_url(source: &Source) -> Result<Url, Box<dyn Error + Send + Sync>> {
    let base_url = Url::parse(&source.base_url)?;
    let start_path = source
        .crawl_config
        .get("start_paths")
        .and_then(|paths| paths.as_array())
        .and_then(|paths| paths.first())
        .and_then(|path| path.as_str())
        .unwrap_or("");

    Ok(base_url.join(start_path)?)
}

#[cfg(test)]
mod tests {
    use policy_shared::{Region, Source};
    use serde_json::json;

    use reqwest::StatusCode;

    use super::{is_retryable_status, source_fetch_url};

    #[test]
    fn fetch_url_uses_the_first_configured_start_path() {
        let source = Source {
            id: 1,
            region: Region::UnitedStates,
            category: policy_shared::SourceCategory::Policy,
            agency: "Example agency".to_owned(),
            base_url: "https://example.gov".to_owned(),
            crawl_config: json!({ "start_paths": ["/policy/feed"] }),
            enabled: true,
        };

        assert_eq!(
            source_fetch_url(&source).unwrap().as_str(),
            "https://example.gov/policy/feed"
        );
    }

    #[test]
    fn temporary_http_responses_are_retried_but_bad_requests_are_not() {
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
    }
}
