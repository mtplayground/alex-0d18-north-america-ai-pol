//! HTTP retrieval for configured policy and news sources.

use std::{error::Error, time::Duration};

use policy_shared::Source;
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client, StatusCode, Url,
};

const MAX_FETCH_ATTEMPTS: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const DOCUMENT_ACCEPT_HEADER: &str = "text/html, application/xhtml+xml, application/json, \
    application/feed+json, application/rss+xml, application/atom+xml, application/xml, text/xml";

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

    /// Fetches every configured start document, retrying temporary source failures.
    pub async fn fetch_all(
        &self,
        source: &Source,
    ) -> Result<Vec<FetchedPayload>, Box<dyn Error + Send + Sync>> {
        let urls = source_fetch_urls(source)?;
        let mut payloads = Vec::with_capacity(urls.len());

        for url in urls {
            payloads.push(self.fetch_url(url).await?);
        }

        Ok(payloads)
    }

    async fn fetch_url(&self, url: Url) -> Result<FetchedPayload, Box<dyn Error + Send + Sync>> {
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
            .header(ACCEPT, DOCUMENT_ACCEPT_HEADER)
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

fn source_fetch_urls(source: &Source) -> Result<Vec<Url>, Box<dyn Error + Send + Sync>> {
    let base_url = Url::parse(&source.base_url)?;
    let start_paths = source
        .crawl_config
        .get("start_paths")
        .and_then(|paths| paths.as_array())
        .filter(|paths| !paths.is_empty());

    let Some(start_paths) = start_paths else {
        return Ok(vec![base_url]);
    };

    start_paths
        .iter()
        .enumerate()
        .map(|(index, start_path)| {
            let start_path = start_path.as_str().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("crawl_config.start_paths[{index}] must be a string"),
                )
            })?;
            Ok(base_url.join(start_path)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use policy_shared::{Region, Source};
    use serde_json::json;

    use reqwest::{StatusCode, Url};

    use super::{is_retryable_status, source_fetch_urls, DOCUMENT_ACCEPT_HEADER};

    #[test]
    fn fetch_urls_include_every_configured_start_path_in_order() {
        let source = Source {
            id: 1,
            region: Region::UnitedStates,
            category: policy_shared::SourceCategory::Policy,
            agency: "Example agency".to_owned(),
            base_url: "https://example.gov".to_owned(),
            crawl_config: json!({ "start_paths": ["/policy/feed", "/policy/archive"] }),
            enabled: true,
        };

        assert_eq!(
            source_fetch_urls(&source)
                .unwrap()
                .iter()
                .map(Url::as_str)
                .collect::<Vec<_>>(),
            vec![
                "https://example.gov/policy/feed",
                "https://example.gov/policy/archive",
            ]
        );
    }

    #[test]
    fn fetch_accept_header_requests_policy_and_feed_media_types() {
        for media_type in [
            "text/html",
            "application/json",
            "application/feed+json",
            "application/rss+xml",
            "application/atom+xml",
            "application/xml",
            "text/xml",
        ] {
            assert!(DOCUMENT_ACCEPT_HEADER.contains(media_type));
        }
    }

    #[test]
    fn temporary_http_responses_are_retried_but_bad_requests_are_not() {
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
    }
}
