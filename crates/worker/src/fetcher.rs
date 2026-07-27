//! HTTP retrieval and bounded discovery for configured policy and news sources.

use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    io,
    time::Duration,
};

use policy_shared::{CrawlConfig, Source};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use regex::Regex;
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client, StatusCode, Url,
};
use scraper::{Html, Selector};

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

    /// Fetches configured entry documents and, when bounded discovery is
    /// enabled, same-host links found in HTML, sitemap, RSS, and Atom payloads.
    pub async fn fetch_all(
        &self,
        source: &Source,
    ) -> Result<Vec<FetchedPayload>, Box<dyn Error + Send + Sync>> {
        let plan = CrawlPlan::from_source(source)?;
        let mut queue = VecDeque::from(
            plan.entry_candidates()?
                .into_iter()
                .take(plan.max_pages)
                .collect::<Vec<_>>(),
        );
        let mut queued: HashSet<String> = queue
            .iter()
            .map(|candidate| url_key(&candidate.url))
            .collect();
        let mut visited = HashSet::new();
        let mut payloads = Vec::with_capacity(queue.len().min(plan.max_pages));

        while let Some(candidate) = queue.pop_front() {
            if payloads.len() == plan.max_pages {
                break;
            }

            let key = url_key(&candidate.url);
            if !visited.insert(key) {
                continue;
            }

            let payload = self.fetch_url(candidate.url).await?;
            if plan.discovery_enabled() && candidate.depth < plan.max_depth {
                for url in plan.discovered_urls(&payload)? {
                    let key = url_key(&url);
                    if queued.len() < plan.max_pages && !visited.contains(&key) && queued.insert(key) {
                        queue.push_back(CrawlCandidate {
                            url,
                            depth: candidate.depth.saturating_add(1),
                        });
                    }
                }
            }
            payloads.push(payload);
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
            .get(url)
            .header(ACCEPT, DOCUMENT_ACCEPT_HEADER)
            .send()
            .await
            .map_err(FetchError::Request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::Status(status));
        }

        let response_url = response.url().clone();
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
            url: response_url,
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

#[derive(Clone, Debug)]
struct CrawlCandidate {
    url: Url,
    depth: usize,
}

struct CrawlPlan {
    base_url: Url,
    config: CrawlConfig,
    include_patterns: Vec<Regex>,
    exclude_patterns: Vec<Regex>,
    max_pages: usize,
    max_depth: usize,
}

impl CrawlPlan {
    fn from_source(source: &Source) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let config = CrawlConfig::parse(&source.crawl_config)?;
        let base_url = Url::parse(&source.base_url)?;
        let include_patterns = compile_patterns("include_patterns", &config.include_patterns)?;
        let exclude_patterns = compile_patterns("exclude_patterns", &config.exclude_patterns)?;
        let (max_pages, max_depth) = match (config.max_pages, config.max_depth) {
            (Some(max_pages), Some(max_depth)) => (max_pages, max_depth),
            (None, None) => {
                // No crawl bounds means exact legacy behavior: fetch only starts.
                (usize::MAX, 0)
            }
            _ => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "crawl_config bounds must be validated before building a crawl plan",
                )));
            }
        };

        Ok(Self {
            base_url,
            config,
            include_patterns,
            exclude_patterns,
            max_pages,
            max_depth,
        })
    }

    fn discovery_enabled(&self) -> bool {
        self.config.discovery_enabled()
    }

    fn entry_candidates(&self) -> Result<Vec<CrawlCandidate>, Box<dyn Error + Send + Sync>> {
        let mut paths = self.config.start_paths.clone();
        if paths.is_empty() {
            paths.push(String::new());
        }
        if self.discovery_enabled() {
            paths.extend(self.config.sitemap_paths.iter().cloned());
            paths.extend(self.config.feed_paths.iter().cloned());
        }

        let mut unique = HashSet::new();
        paths
            .into_iter()
            .map(|path| self.configured_url(&path))
            .filter(|result| match result {
                Ok(url) => unique.insert(url_key(url)),
                Err(_) => true,
            })
            .map(|result| result.map(|url| CrawlCandidate { url, depth: 0 }))
            .collect()
    }

    fn configured_url(&self, path: &str) -> Result<Url, Box<dyn Error + Send + Sync>> {
        let url = normalize_url(self.base_url.join(path)?);
        if !is_source_url(&self.base_url, &url) {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("configured URL must stay on source host: {url}"),
            )));
        }
        Ok(url)
    }

    fn discovered_urls(
        &self,
        payload: &FetchedPayload,
    ) -> Result<Vec<Url>, Box<dyn Error + Send + Sync>> {
        let candidates = extract_discovery_links(payload)?;
        let mut unique = HashSet::new();

        Ok(candidates
            .into_iter()
            .filter_map(|candidate| payload.url.join(&candidate).ok())
            .map(normalize_url)
            .filter(|url| self.allows_discovered_url(url))
            .filter(|url| unique.insert(url_key(url)))
            .collect())
    }

    fn allows_discovered_url(&self, url: &Url) -> bool {
        is_source_url(&self.base_url, url)
            && (self.config.allowed_path_prefixes.is_empty()
                || self
                    .config
                    .allowed_path_prefixes
                    .iter()
                    .any(|prefix| url.path().starts_with(prefix)))
            && (self.include_patterns.is_empty()
                || self.include_patterns.iter().any(|pattern| pattern.is_match(url.as_str())))
            && !self
                .exclude_patterns
                .iter()
                .any(|pattern| pattern.is_match(url.as_str()))
    }
}

fn compile_patterns(
    field: &str,
    patterns: &[String],
) -> Result<Vec<Regex>, Box<dyn Error + Send + Sync>> {
    patterns
        .iter()
        .enumerate()
        .map(|(index, pattern)| {
            Regex::new(pattern).map_err(|error| {
                Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("crawl_config.{field}[{index}] is not a valid regular expression: {error}"),
                )) as Box<dyn Error + Send + Sync>
            })
        })
        .collect()
}

fn extract_discovery_links(
    payload: &FetchedPayload,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    if is_html(&payload.content_type) {
        return Ok(html_links(&payload.body));
    }
    if is_xml(&payload.content_type) {
        return xml_links(&payload.body).map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>);
    }

    Ok(Vec::new())
}

fn html_links(body: &[u8]) -> Vec<String> {
    let document = Html::parse_document(&String::from_utf8_lossy(body));
    let Ok(selector) = Selector::parse("a[href]") else {
        return Vec::new();
    };

    document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .map(ToOwned::to_owned)
        .collect()
}

fn xml_links(body: &[u8]) -> Result<Vec<String>, quick_xml::Error> {
    let mut reader = Reader::from_reader(body);
    reader.trim_text(true);
    let mut buffer = Vec::new();
    let mut links = Vec::new();
    let mut article_depth = 0_usize;
    let mut capture_text = None;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) => {
                let tag = xml_tag_name(element.name().as_ref());
                if matches!(tag.as_str(), "item" | "entry") {
                    article_depth = article_depth.saturating_add(1);
                }
                if tag == "loc" || (tag == "link" && article_depth > 0) {
                    if tag == "link" {
                        if let Some(href) = xml_attribute(&element, "href")? {
                            links.push(href);
                        } else {
                            capture_text = Some(tag);
                        }
                    } else {
                        capture_text = Some(tag);
                    }
                }
            }
            Event::Empty(element) => {
                let tag = xml_tag_name(element.name().as_ref());
                if tag == "link" && article_depth > 0 {
                    if let Some(href) = xml_attribute(&element, "href")? {
                        links.push(href);
                    }
                }
            }
            Event::Text(text) => {
                if capture_text.is_some() {
                    let value = text.unescape()?.trim().to_owned();
                    if !value.is_empty() {
                        links.push(value);
                    }
                }
            }
            Event::CData(text) => {
                if capture_text.is_some() {
                    let value = String::from_utf8_lossy(text.as_ref()).trim().to_owned();
                    if !value.is_empty() {
                        links.push(value);
                    }
                }
            }
            Event::End(element) => {
                let tag = xml_tag_name(element.name().as_ref());
                if capture_text.as_deref() == Some(tag.as_str()) {
                    capture_text = None;
                }
                if matches!(tag.as_str(), "item" | "entry") {
                    article_depth = article_depth.saturating_sub(1);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    Ok(links)
}

fn xml_attribute(element: &BytesStart<'_>, name: &str) -> Result<Option<String>, quick_xml::Error> {
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute?;
        if xml_tag_name(attribute.key.as_ref()) == name {
            return attribute.unescape_value().map(|value| Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn xml_tag_name(value: &[u8]) -> String {
    String::from_utf8_lossy(value)
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_html(content_type: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();
    content_type.contains("text/html") || content_type.contains("application/xhtml+xml")
}

fn is_xml(content_type: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();
    content_type.contains("xml") || content_type.contains("rss") || content_type.contains("atom")
}

fn normalize_url(mut url: Url) -> Url {
    url.set_fragment(None);
    url
}

fn url_key(url: &Url) -> String {
    normalize_url(url.clone()).to_string()
}

fn is_source_url(base_url: &Url, candidate: &Url) -> bool {
    matches!(candidate.scheme(), "http" | "https")
        && base_url.host_str() == candidate.host_str()
        && base_url.port_or_known_default() == candidate.port_or_known_default()
}

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

#[cfg(test)]
fn source_fetch_urls(source: &Source) -> Result<Vec<Url>, Box<dyn Error + Send + Sync>> {
    CrawlPlan::from_source(source)?
        .entry_candidates()
        .map(|candidates| candidates.into_iter().map(|candidate| candidate.url).collect())
}

#[cfg(test)]
mod tests {
    use policy_shared::{Region, Source, SourceCategory};
    use reqwest::{StatusCode, Url};
    use serde_json::json;

    use super::{
        extract_discovery_links, is_retryable_status, source_fetch_urls, CrawlPlan, FetchedPayload,
        DOCUMENT_ACCEPT_HEADER,
    };

    fn source(crawl_config: serde_json::Value) -> Source {
        Source {
            id: 1,
            region: Region::UnitedStates,
            category: SourceCategory::Policy,
            agency: "Example agency".to_owned(),
            base_url: "https://example.gov".to_owned(),
            crawl_config,
            enabled: true,
        }
    }

    #[test]
    fn fetch_urls_include_every_configured_start_path_in_order() {
        let source = source(json!({ "start_paths": ["/policy/feed", "/policy/archive"] }));

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
    fn start_only_sources_keep_legacy_fetch_roots() {
        let source = source(json!({
            "start_paths": ["/policy/feed"],
            "sitemap_paths": ["/sitemap.xml"],
            "feed_paths": ["/news.xml"]
        }));

        assert_eq!(
            source_fetch_urls(&source)
                .unwrap()
                .iter()
                .map(Url::as_str)
                .collect::<Vec<_>>(),
            vec!["https://example.gov/policy/feed"]
        );
    }

    #[test]
    fn configured_discovery_roots_are_deduplicated_and_bounded() {
        let source = source(json!({
            "start_paths": ["/index", "/index#top"],
            "sitemap_paths": ["/sitemap.xml"],
            "feed_paths": ["/feed.xml"],
            "max_pages": 3,
            "max_depth": 2
        }));
        let plan = CrawlPlan::from_source(&source).unwrap();
        let candidates = plan.entry_candidates().unwrap();

        assert_eq!(plan.max_pages, 3);
        assert_eq!(plan.max_depth, 2);
        assert_eq!(
            candidates.into_iter().map(|candidate| candidate.url.to_string()).collect::<Vec<_>>(),
            vec![
                "https://example.gov/index",
                "https://example.gov/sitemap.xml",
                "https://example.gov/feed.xml",
            ]
        );
    }

    #[test]
    fn discovered_html_links_stay_on_host_and_obey_all_filters() {
        let source = source(json!({
            "allowed_path_prefixes": ["/news/"],
            "include_patterns": ["/202[56]/"],
            "exclude_patterns": ["draft"],
            "max_pages": 10,
            "max_depth": 2
        }));
        let plan = CrawlPlan::from_source(&source).unwrap();
        let payload = FetchedPayload {
            url: Url::parse("https://example.gov/news/index").unwrap(),
            content_type: "text/html; charset=utf-8".to_owned(),
            body: br#"
                <a href="/news/2026/released#details">released</a>
                <a href="/news/2026/draft">draft</a>
                <a href="/other/2026/ignored">other</a>
                <a href="https://other.example/news/2026/ignored">other host</a>
                <a href="mailto:team@example.gov">mail</a>
            "#
            .to_vec(),
        };

        assert_eq!(
            plan.discovered_urls(&payload)
                .unwrap()
                .into_iter()
                .map(|url| url.to_string())
                .collect::<Vec<_>>(),
            vec!["https://example.gov/news/2026/released"]
        );
    }

    #[test]
    fn sitemap_rss_and_atom_links_are_extracted() {
        let sitemap = FetchedPayload {
            url: Url::parse("https://example.gov/sitemap.xml").unwrap(),
            content_type: "application/xml".to_owned(),
            body: br#"<urlset><url><loc>/news/one</loc></url><url><loc>https://example.gov/news/two</loc></url></urlset>"#.to_vec(),
        };
        let feed = FetchedPayload {
            url: Url::parse("https://example.gov/feed.xml").unwrap(),
            content_type: "application/atom+xml".to_owned(),
            body: br#"<feed><entry><link href="/news/three" /></entry><item><link>/news/four</link></item></feed>"#.to_vec(),
        };

        assert_eq!(extract_discovery_links(&sitemap).unwrap(), ["/news/one", "https://example.gov/news/two"]);
        assert_eq!(extract_discovery_links(&feed).unwrap(), ["/news/three", "/news/four"]);
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
