//! HTTP retrieval for configured government sources.

use std::{error::Error, time::Duration};

use policy_shared::Source;
use reqwest::{header::CONTENT_TYPE, Client, Url};

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

    /// Fetches the configured start document and rejects non-success responses.
    pub async fn fetch(
        &self,
        source: &Source,
    ) -> Result<FetchedPayload, Box<dyn Error + Send + Sync>> {
        let url = source_fetch_url(source)?;
        let response = self.client.get(url.clone()).send().await?.error_for_status()?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let body = response.bytes().await?.to_vec();

        Ok(FetchedPayload {
            url,
            content_type,
            body,
        })
    }
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

    use super::source_fetch_url;

    #[test]
    fn fetch_url_uses_the_first_configured_start_path() {
        let source = Source {
            id: 1,
            region: Region::UnitedStates,
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
}
