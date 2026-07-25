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
}

impl ChangeFeedQuery {
    /// Returns validated, bounded pagination values.
    pub fn pagination(&self) -> (u32, u32) {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        (limit, self.offset.unwrap_or_default())
    }
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
            }
            .pagination(),
            (100, 7)
        );
    }
}
