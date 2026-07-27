//! Typed parsing and validation for a source's flexible crawler configuration.

use std::{error::Error, fmt};

use serde::Deserialize;
use serde_json::Value;

/// Bounded discovery settings stored in `sources.crawl_config`.
///
/// A source without both crawl bounds remains a start-URLs-only source. This
/// makes discovery opt-in for existing source rows.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct CrawlConfig {
    /// Explicit documents fetched for every run.
    pub start_paths: Vec<String>,
    /// Explicit sitemap documents used as discovery entry points.
    pub sitemap_paths: Vec<String>,
    /// Explicit RSS or Atom documents used as discovery entry points.
    pub feed_paths: Vec<String>,
    /// Allowed path prefixes for URLs found during discovery.
    pub allowed_path_prefixes: Vec<String>,
    /// Regular expressions that a discovered URL must match when present.
    pub include_patterns: Vec<String>,
    /// Regular expressions that reject a discovered URL.
    pub exclude_patterns: Vec<String>,
    /// Maximum total documents fetched during a discovery-enabled source run.
    pub max_pages: Option<usize>,
    /// Maximum discovered-link distance from an explicit entry point.
    pub max_depth: Option<usize>,
}

impl CrawlConfig {
    /// Parses and validates crawl configuration from the database JSON value.
    pub fn parse(value: &Value) -> Result<Self, CrawlConfigError> {
        let config: Self = serde_json::from_value(value.clone())?;
        config.validate()?;
        Ok(config)
    }

    /// Whether bounded discovery is explicitly enabled for this source.
    pub const fn discovery_enabled(&self) -> bool {
        self.max_pages.is_some() && self.max_depth.is_some()
    }

    fn validate(&self) -> Result<(), CrawlConfigError> {
        match (self.max_pages, self.max_depth) {
            (None, None) => Ok(()),
            (Some(0), _) => Err(CrawlConfigError::InvalidBounds(
                "max_pages must be greater than zero".to_owned(),
            )),
            (Some(_), Some(_)) => Ok(()),
            _ => Err(CrawlConfigError::InvalidBounds(
                "max_pages and max_depth must be configured together".to_owned(),
            )),
        }
    }
}

/// An invalid source crawler configuration.
#[derive(Debug)]
pub enum CrawlConfigError {
    /// The JSON shape did not match the crawler contract.
    Deserialize(serde_json::Error),
    /// Discovery bounds were incomplete or unsafe.
    InvalidBounds(String),
}

impl fmt::Display for CrawlConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deserialize(error) => write!(formatter, "invalid crawl_config: {error}"),
            Self::InvalidBounds(message) => write!(formatter, "invalid crawl_config bounds: {message}"),
        }
    }
}

impl Error for CrawlConfigError {}

impl From<serde_json::Error> for CrawlConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Deserialize(error)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::CrawlConfig;

    #[test]
    fn discovery_requires_both_hard_bounds() {
        assert!(CrawlConfig::parse(&json!({ "max_pages": 10 })).is_err());
        assert!(CrawlConfig::parse(&json!({ "max_depth": 2 })).is_err());
        assert!(CrawlConfig::parse(&json!({ "max_pages": 0, "max_depth": 2 })).is_err());
    }

    #[test]
    fn existing_start_only_configuration_stays_valid() {
        let config = CrawlConfig::parse(&json!({ "start_paths": ["/feed"] })).unwrap();

        assert!(!config.discovery_enabled());
        assert_eq!(config.start_paths, ["/feed"]);
    }

    #[test]
    fn bounded_discovery_configuration_preserves_all_url_filters() {
        let config = CrawlConfig::parse(&json!({
            "sitemap_paths": ["/sitemap.xml"],
            "feed_paths": ["/feed.xml"],
            "allowed_path_prefixes": ["/news/"],
            "include_patterns": ["/202[56]/"],
            "exclude_patterns": ["/tag/"],
            "max_pages": 20,
            "max_depth": 2
        }))
        .unwrap();

        assert!(config.discovery_enabled());
        assert_eq!(config.sitemap_paths, ["/sitemap.xml"]);
        assert_eq!(config.feed_paths, ["/feed.xml"]);
    }
}
