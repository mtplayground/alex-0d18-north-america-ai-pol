//! DTOs for the reverse-chronological policy change feed.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

/// Query parameters accepted by the change-feed endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct ChangeFeedQuery {
    /// Maximum number of items to return, capped at 100.
    pub limit: Option<u32>,
    /// Number of most-recent items to skip.
    pub offset: Option<u32>,
    /// Jurisdiction code to include, such as `us` or `ca`.
    pub region: Option<String>,
    /// Publishing agency to include.
    pub agency: Option<String>,
    /// Current policy status to include.
    pub status: Option<String>,
}

impl ChangeFeedQuery {
    /// Returns validated, bounded pagination values.
    pub fn pagination(&self) -> (u32, u32) {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        (limit, self.offset.unwrap_or_default())
    }

    /// Returns optional, normalized values for feed refinement.
    pub fn filters(&self) -> (Option<String>, Option<String>, Option<String>) {
        (
            normalized_filter(self.region.as_deref()).map(str::to_ascii_lowercase),
            normalized_filter(self.agency.as_deref()).map(str::to_owned),
            normalized_filter(self.status.as_deref()).map(str::to_owned),
        )
    }
}

fn normalized_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// One policy entry in the change feed, represented by its latest version.
#[derive(Clone, Debug, Serialize)]
pub struct ChangeFeedItem {
    pub title: String,
    pub region: String,
    pub agency: String,
    pub publication_date: Option<NaiveDate>,
    pub status: String,
    pub source_url: String,
    pub change_summary: Option<String>,
    pub changed_at: DateTime<Utc>,
}

/// A page of the latest observed changes across policy entries.
#[derive(Clone, Debug, Serialize)]
pub struct ChangeFeedResponse {
    pub items: Vec<ChangeFeedItem>,
    pub limit: u32,
    pub offset: u32,
    pub next_offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::ChangeFeedQuery;

    #[test]
    fn pagination_uses_defaults_and_bounds_the_limit() {
        assert_eq!(ChangeFeedQuery::default().pagination(), (20, 0));
        assert_eq!(
            ChangeFeedQuery {
                limit: Some(500),
                offset: Some(7),
                ..ChangeFeedQuery::default()
            }
            .pagination(),
            (100, 7)
        );
    }

    #[test]
    fn filters_trim_empty_values_and_normalize_region() {
        let query = ChangeFeedQuery {
            region: Some(" CA ".to_owned()),
            agency: Some("  Health Canada  ".to_owned()),
            status: Some(" ".to_owned()),
            ..ChangeFeedQuery::default()
        };

        assert_eq!(
            query.filters(),
            (Some("ca".to_owned()), Some("Health Canada".to_owned()), None)
        );
    }
}
