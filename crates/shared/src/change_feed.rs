//! DTOs for the sortable policy change feed.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

/// Supported sort orders for the change feed.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeFeedSort {
    /// Most recently published entries first, with unknown publication dates last.
    #[default]
    PublishedDesc,
    /// Oldest published entries first, with unknown publication dates last.
    PublishedAsc,
    /// Most recently observed crawler changes first.
    ObservedDesc,
    /// Oldest observed crawler changes first.
    ObservedAsc,
}

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
    /// Source category to include, such as `policy` or `news`.
    pub category: Option<String>,
    /// Free-text term matched against entry titles and change summaries.
    pub q: Option<String>,
    /// Ordering of items in the response. Missing values use newest publication first.
    pub sort: Option<ChangeFeedSort>,
}

impl ChangeFeedQuery {
    /// Returns validated, bounded pagination values.
    pub fn pagination(&self) -> (u32, u32) {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        (limit, self.offset.unwrap_or_default())
    }

    /// Returns optional, normalized values for feed refinement.
    pub fn filters(
        &self,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        (
            normalized_filter(self.region.as_deref()).map(str::to_ascii_lowercase),
            normalized_filter(self.agency.as_deref()).map(str::to_owned),
            normalized_filter(self.status.as_deref()).map(str::to_owned),
            normalized_filter(self.category.as_deref()).map(str::to_ascii_lowercase),
        )
    }

    /// Returns an optional trimmed free-text search term.
    ///
    /// Blank and absent values intentionally behave the same so clients can
    /// keep an empty search field in their URL without changing the feed.
    pub fn search_term(&self) -> Option<String> {
        normalized_filter(self.q.as_deref()).map(str::to_owned)
    }

    /// Returns the requested feed order, defaulting to newest publication first.
    pub fn sort(&self) -> ChangeFeedSort {
        self.sort.unwrap_or_default()
    }
}

fn normalized_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// One policy entry in the change feed, represented by its latest version.
#[derive(Clone, Debug, Serialize)]
pub struct ChangeFeedItem {
    /// Identifier used to request the expandable entry detail.
    pub entry_id: i64,
    pub title: String,
    pub region: String,
    /// Category of the source that published this entry.
    pub source_category: String,
    pub agency: String,
    pub publication_date: Option<NaiveDate>,
    /// True when the publication date is after the current date.
    pub scheduled: bool,
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
    use chrono::{DateTime, NaiveDate, Utc};
    use serde_json::json;

    use super::{ChangeFeedItem, ChangeFeedQuery, ChangeFeedSort};

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
    fn filters_trim_empty_values_and_normalize_codes() {
        let query = ChangeFeedQuery {
            region: Some(" CA ".to_owned()),
            agency: Some("  Health Canada  ".to_owned()),
            status: Some(" ".to_owned()),
            category: Some(" NEWS ".to_owned()),
            ..ChangeFeedQuery::default()
        };

        assert_eq!(
            query.filters(),
            (
                Some("ca".to_owned()),
                Some("Health Canada".to_owned()),
                None,
                Some("news".to_owned()),
            )
        );
    }

    #[test]
    fn search_term_trims_values_and_ignores_blank_queries() {
        assert_eq!(ChangeFeedQuery::default().search_term(), None);

        let blank = ChangeFeedQuery {
            q: Some("  \t ".to_owned()),
            ..ChangeFeedQuery::default()
        };
        assert_eq!(blank.search_term(), None);

        let query = ChangeFeedQuery {
            q: Some("  artificial intelligence  ".to_owned()),
            ..ChangeFeedQuery::default()
        };
        assert_eq!(query.search_term(), Some("artificial intelligence".to_owned()));
    }

    #[test]
    fn sort_defaults_to_newest_published_and_accepts_each_wire_value() {
        assert_eq!(
            ChangeFeedQuery::default().sort(),
            ChangeFeedSort::PublishedDesc
        );

        for (wire_value, expected) in [
            ("published_desc", ChangeFeedSort::PublishedDesc),
            ("published_asc", ChangeFeedSort::PublishedAsc),
            ("observed_desc", ChangeFeedSort::ObservedDesc),
            ("observed_asc", ChangeFeedSort::ObservedAsc),
        ] {
            let query: ChangeFeedQuery =
                serde_json::from_value(json!({ "sort": wire_value })).expect("valid sort");

            assert_eq!(query.sort(), expected);
        }
    }

    #[test]
    fn change_feed_item_serializes_scheduled_state() {
        let changed_at = DateTime::<Utc>::from_timestamp(0, 0).expect("valid Unix timestamp");
        let publication_date = NaiveDate::from_ymd_opt(2026, 7, 30).expect("valid date");
        let item = ChangeFeedItem {
            entry_id: 12,
            title: "Scheduled item".to_owned(),
            region: "us".to_owned(),
            source_category: "policy".to_owned(),
            agency: "Example agency".to_owned(),
            publication_date: Some(publication_date),
            scheduled: true,
            status: "published".to_owned(),
            source_url: "https://example.test/scheduled".to_owned(),
            change_summary: None,
            changed_at,
        };

        let value = serde_json::to_value(item).expect("feed item serializes");
        assert_eq!(value["scheduled"], true);
    }
}
