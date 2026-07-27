//! HTTP retrieval and bounded discovery for configured policy and news sources.

use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    io,
    sync::Arc,
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
use tokio::{
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

const MAX_FETCH_ATTEMPTS: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const CRAWLER_USER_AGENT: &str = "policy-source-crawler/1.0";
const HOST_REQUEST_DELAY: Duration = Duration::from_millis(250);
const HOST_MAX_CONCURRENT_REQUESTS: usize = 2;
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
    robots_cache: RobotsCache,
    host_limiter: HostRateLimiter,
}

impl SourceFetcher {
    /// Creates a client with a timeout appropriate for scheduled ingestion.
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent(CRAWLER_USER_AGENT)
                .build()?,
            robots_cache: RobotsCache::default(),
            host_limiter: HostRateLimiter::new(HOST_REQUEST_DELAY, HOST_MAX_CONCURRENT_REQUESTS),
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

            if candidate.discovered
                && !self
                    .robots_cache
                    .allows(&self.client, &self.host_limiter, &candidate.url)
                    .await
            {
                eprintln!(
                    "skipping discovered URL {} because robots.txt disallows it for {CRAWLER_USER_AGENT}",
                    candidate.url
                );
                continue;
            }

            let payload = self.fetch_url(candidate.url, candidate.discovered).await?;
            if plan.discovery_enabled() && candidate.depth < plan.max_depth {
                for url in plan.discovered_urls(&payload)? {
                    let key = url_key(&url);
                    if queued.len() < plan.max_pages && !visited.contains(&key) && queued.insert(key) {
                        queue.push_back(CrawlCandidate {
                            url,
                            depth: candidate.depth.saturating_add(1),
                            discovered: true,
                        });
                    }
                }
            }
            payloads.push(payload);
        }

        Ok(payloads)
    }

    async fn fetch_url(
        &self,
        url: Url,
        rate_limit: bool,
    ) -> Result<FetchedPayload, Box<dyn Error + Send + Sync>> {
        let _host_permit = if rate_limit {
            Some(self.host_limiter.acquire(&url).await?)
        } else {
            None
        };
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

#[derive(Clone, Default)]
struct RobotsCache {
    policies: Arc<Mutex<std::collections::HashMap<String, RobotsPolicy>>>,
}

impl RobotsCache {
    async fn allows(&self, client: &Client, limiter: &HostRateLimiter, url: &Url) -> bool {
        let host = host_key(url);
        if let Some(policy) = self.policies.lock().await.get(&host).cloned() {
            return policy.allows(url);
        }

        let policy = fetch_robots_policy(client, limiter, url).await;
        let allows = policy.allows(url);
        self.policies.lock().await.insert(host, policy);
        allows
    }
}

#[derive(Clone)]
enum RobotsPolicy {
    Rules(Vec<RobotsRule>),
    AllowAll,
    DenyAll,
}

impl RobotsPolicy {
    fn allows(&self, url: &Url) -> bool {
        match self {
            Self::Rules(rules) => robots_allows(rules, url),
            Self::AllowAll => true,
            Self::DenyAll => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RobotsRule {
    pattern: String,
    allow: bool,
}

async fn fetch_robots_policy(
    client: &Client,
    limiter: &HostRateLimiter,
    source_url: &Url,
) -> RobotsPolicy {
    let mut robots_url = source_url.clone();
    robots_url.set_path("/robots.txt");
    robots_url.set_query(None);
    robots_url.set_fragment(None);

    let permit = match limiter.acquire(&robots_url).await {
        Ok(permit) => permit,
        Err(error) => {
            eprintln!("cannot pace robots.txt request for {robots_url}: {error}");
            return RobotsPolicy::DenyAll;
        }
    };
    let response = client.get(robots_url.clone()).send().await;
    drop(permit);

    match response {
        Ok(response) if response.status().is_success() => match response.text().await {
            Ok(body) => RobotsPolicy::Rules(parse_robots_rules(&body, CRAWLER_USER_AGENT)),
            Err(error) => {
                eprintln!("cannot read robots.txt at {robots_url}: {error}; skipping discovered URLs");
                RobotsPolicy::DenyAll
            }
        },
        Ok(response) if matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE) => {
            RobotsPolicy::AllowAll
        }
        Ok(response) => {
            eprintln!(
                "robots.txt at {robots_url} returned {}; skipping discovered URLs",
                response.status()
            );
            RobotsPolicy::DenyAll
        }
        Err(error) => {
            eprintln!("cannot fetch robots.txt at {robots_url}: {error}; skipping discovered URLs");
            RobotsPolicy::DenyAll
        }
    }
}

fn parse_robots_rules(body: &str, user_agent: &str) -> Vec<RobotsRule> {
    let mut groups = Vec::new();
    let mut agents = Vec::new();
    let mut rules = Vec::new();

    for line in body.lines() {
        let line = line.split_once('#').map_or(line, |(value, _)| value).trim();
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();

        if name == "user-agent" {
            if !rules.is_empty() {
                groups.push((agents, rules));
                agents = Vec::new();
                rules = Vec::new();
            }
            if !value.is_empty() {
                agents.push(value.to_ascii_lowercase());
            }
        } else if matches!(name.as_str(), "allow" | "disallow") && !agents.is_empty() && !value.is_empty() {
            rules.push(RobotsRule {
                pattern: value.to_owned(),
                allow: name == "allow",
            });
        }
    }
    if !agents.is_empty() {
        groups.push((agents, rules));
    }

    let user_agent = user_agent.to_ascii_lowercase();
    let best_match = groups
        .iter()
        .flat_map(|(agents, _)| agents)
        .filter_map(|agent| robots_agent_match(agent, &user_agent))
        .max()
        .unwrap_or(0);

    groups
        .into_iter()
        .filter(|(agents, _)| {
            agents
                .iter()
                .any(|agent| robots_agent_match(agent, &user_agent) == Some(best_match))
        })
        .flat_map(|(_, rules)| rules)
        .collect()
}

fn robots_agent_match(agent: &str, user_agent: &str) -> Option<usize> {
    if agent == "*" {
        Some(0)
    } else if user_agent.starts_with(agent) {
        Some(agent.len())
    } else {
        None
    }
}

fn robots_allows(rules: &[RobotsRule], url: &Url) -> bool {
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    };
    let mut best_rule: Option<(&RobotsRule, usize)> = None;

    for rule in rules {
        let Some(specificity) = robots_rule_matches(&rule.pattern, &path) else {
            continue;
        };
        if best_rule.is_none_or(|(current, current_specificity)| {
            specificity > current_specificity
                || (specificity == current_specificity && rule.allow && !current.allow)
        }) {
            best_rule = Some((rule, specificity));
        }
    }

    best_rule.is_none_or(|(rule, _)| rule.allow)
}

fn robots_rule_matches(pattern: &str, path: &str) -> Option<usize> {
    let pattern = pattern.trim();
    let anchored = pattern.ends_with('$');
    let pattern = pattern.trim_end_matches('$');
    let expression = format!(
        "^{}{}",
        regex::escape(pattern).replace(r"\*", ".*"),
        if anchored { "$" } else { ".*" }
    );
    let regex = Regex::new(&expression).ok()?;
    regex.is_match(path).then_some(pattern.replace('*', "").len())
}

#[derive(Clone)]
struct HostRateLimiter {
    hosts: Arc<Mutex<std::collections::HashMap<String, HostRateState>>>,
    delay: Duration,
    max_concurrent: usize,
}

struct HostRateState {
    next_request_at: Instant,
    permits: Arc<Semaphore>,
}

struct HostRequestPermit {
    _permit: OwnedSemaphorePermit,
}

impl HostRateLimiter {
    fn new(delay: Duration, max_concurrent: usize) -> Self {
        Self {
            hosts: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delay,
            max_concurrent: max_concurrent.max(1),
        }
    }

    async fn acquire(&self, url: &Url) -> Result<HostRequestPermit, Box<dyn Error + Send + Sync>> {
        let host = host_key(url);
        let permits = {
            let mut hosts = self.hosts.lock().await;
            hosts
                .entry(host.clone())
                .or_insert_with(|| HostRateState {
                    next_request_at: Instant::now(),
                    permits: Arc::new(Semaphore::new(self.max_concurrent)),
                })
                .permits
                .clone()
        };
        let permit = permits.acquire_owned().await.map_err(|_| {
            Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("host request limiter was closed for {host}"),
            )) as Box<dyn Error + Send + Sync>
        })?;
        let wait_until = {
            let mut hosts = self.hosts.lock().await;
            let state = hosts.get_mut(&host).ok_or_else(|| {
                Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("missing host request limiter state for {host}"),
                )) as Box<dyn Error + Send + Sync>
            })?;
            let now = Instant::now();
            let wait_until = state.next_request_at.max(now);
            state.next_request_at = wait_until + self.delay;
            wait_until
        };
        tokio::time::sleep_until(wait_until).await;

        Ok(HostRequestPermit { _permit: permit })
    }
}

fn host_key(url: &Url) -> String {
    format!(
        "{}://{}:{}",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default()
    )
}

#[derive(Clone, Debug)]
struct CrawlCandidate {
    url: Url,
    depth: usize,
    /// Configured start/sitemap/feed entries remain first-class; only URLs
    /// found while parsing a fetched payload are subject to robots and pacing.
    discovered: bool,
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
            .map(|result| {
                result.map(|url| CrawlCandidate {
                    url,
                    depth: 0,
                    discovered: false,
                })
            })
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
    use std::time::Duration;

    use policy_shared::{Region, Source, SourceCategory};
    use reqwest::{StatusCode, Url};
    use serde_json::json;
    use tokio::time::timeout;

    use super::{
        extract_discovery_links, is_retryable_status, parse_robots_rules, robots_allows,
        source_fetch_urls, CrawlPlan, FetchedPayload, HostRateLimiter, DOCUMENT_ACCEPT_HEADER,
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
    fn robots_rules_use_the_specific_user_agent_and_longest_matching_path() {
        let rules = parse_robots_rules(
            "\
             User-agent: *\n\
             Disallow: /private\n\
             User-agent: policy-source-crawler\n\
             Disallow: /staff\n\
             Allow: /staff/public\n",
            "policy-source-crawler/1.0",
        );

        assert!(!robots_allows(&rules, &Url::parse("https://example.gov/staff/internal").unwrap()));
        assert!(robots_allows(&rules, &Url::parse("https://example.gov/staff/public/report").unwrap()));
        assert!(robots_allows(&rules, &Url::parse("https://example.gov/private").unwrap()));
    }

    #[tokio::test]
    async fn host_limiter_caps_concurrent_requests() {
        let limiter = HostRateLimiter::new(Duration::ZERO, 1);
        let url = Url::parse("https://example.gov/news").unwrap();
        let first = limiter.acquire(&url).await.unwrap();
        let next_limiter = limiter.clone();
        let next_url = url.clone();
        let mut second = tokio::spawn(async move { next_limiter.acquire(&next_url).await });

        assert!(timeout(Duration::from_millis(20), &mut second).await.is_err());
        drop(first);
        assert!(matches!(
            timeout(Duration::from_millis(100), &mut second).await,
            Ok(Ok(Ok(_)))
        ));
    }

    #[tokio::test]
    async fn host_limiter_spaces_discovered_requests() {
        let limiter = HostRateLimiter::new(Duration::from_millis(20), 2);
        let url = Url::parse("https://example.gov/news").unwrap();
        let first = limiter.acquire(&url).await.unwrap();
        drop(first);

        let started = tokio::time::Instant::now();
        let second = limiter.acquire(&url).await.unwrap();
        assert!(started.elapsed() >= Duration::from_millis(15));
        drop(second);
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
