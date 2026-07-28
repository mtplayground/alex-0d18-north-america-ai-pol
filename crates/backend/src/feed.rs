//! Change-feed query service and HTTP handler.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use policy_shared::{ChangeFeedItem, ChangeFeedQuery, ChangeFeedResponse, ChangeFeedSort};
use sqlx::FromRow;

use crate::db::Database;

/// Returns the latest version of every policy entry in the requested stable order.
pub async fn list_changes(
    State(database): State<Database>,
    Query(query): Query<ChangeFeedQuery>,
) -> Result<Json<ChangeFeedResponse>, FeedError> {
    let (limit, offset) = query.pagination();
    let (region, agency, status, category) = query.filters();
    let search_term = query.search_term();
    let order_by = order_by(query.sort());
    let sql = format!(
        "WITH candidate_rows AS ( \
             SELECT entries.id AS entry_id, entries.title, entries.region, sources.category AS source_category, entries.agency, entries.publication_date, \
                    entries.status, entries.source_url, latest.change_summary, latest.observed_at, latest.id AS latest_version_id, \
                    CASE \
                        WHEN entries.publication_date IS NOT NULL \
                             AND BTRIM(entries.title) <> '' \
                             AND BTRIM(entries.agency) <> '' \
                        THEN CONCAT(sources.category, ':fallback:', LOWER(REGEXP_REPLACE(BTRIM(entries.title), '[[:space:]]+', ' ', 'g')), '|', LOWER(REGEXP_REPLACE(BTRIM(entries.agency), '[[:space:]]+', ' ', 'g')), '|', entries.publication_date::TEXT) \
                        ELSE CONCAT('entry:', entries.id::TEXT) \
                    END AS policy_identity \
             FROM policy_entries AS entries \
             JOIN sources ON sources.id = entries.source_id \
             JOIN LATERAL ( \
                 SELECT id, change_summary, observed_at \
                 FROM policy_versions \
                 WHERE policy_entry_id = entries.id \
                 ORDER BY observed_at DESC, id DESC \
                 LIMIT 1 \
             ) AS latest ON TRUE \
             WHERE ($3::TEXT IS NULL OR entries.region = $3) \
               AND ($4::TEXT IS NULL OR entries.agency = $4) \
               AND ($5::TEXT IS NULL OR entries.status = $5) \
               AND ($6::TEXT IS NULL OR sources.category = $6) \
               AND ( \
                    $7::TEXT IS NULL \
                    OR entries.title ILIKE CONCAT('%', $7, '%') \
                    OR COALESCE(latest.change_summary, '') ILIKE CONCAT('%', $7, '%') \
               ) \
         ), deduplicated_rows AS ( \
             SELECT DISTINCT ON (policy_identity) \
                    entry_id, title, region, source_category, agency, publication_date, status, source_url, change_summary, observed_at, latest_version_id \
             FROM candidate_rows \
             ORDER BY policy_identity, observed_at DESC, latest_version_id DESC, entry_id DESC \
         ) \
         SELECT entry_id, title, region, source_category, agency, publication_date, \
                COALESCE(publication_date > CURRENT_DATE, FALSE) AS scheduled, \
                status, source_url, change_summary, observed_at \
         FROM deduplicated_rows \
         ORDER BY {order_by} \
         LIMIT $1 OFFSET $2"
    );
    let rows = sqlx::query_as::<_, FeedRow>(&sql)
        .bind(i64::from(limit) + 1)
        .bind(i64::from(offset))
        .bind(region)
        .bind(agency)
        .bind(status)
        .bind(category)
        .bind(search_term)
        .fetch_all(&database.pool)
        .await
        .map_err(FeedError::Database)?;

    let has_next_page = rows.len() > usize::try_from(limit).expect("u32 fits in usize");
    let items = rows
        .into_iter()
        .take(usize::try_from(limit).expect("u32 fits in usize"))
        .map(FeedRow::into_item)
        .collect();

    Ok(Json(ChangeFeedResponse {
        items,
        limit,
        offset,
        next_offset: has_next_page.then_some(offset.saturating_add(limit)),
    }))
}

/// Returns a fixed SQL ordering fragment for a validated shared enum value.
///
/// The fragment is never derived from request text, so it remains safe to
/// compose into the otherwise parameterized feed query.
const fn order_by(sort: ChangeFeedSort) -> &'static str {
    match sort {
        ChangeFeedSort::PublishedDesc => {
            "CASE WHEN publication_date > CURRENT_DATE THEN 1 ELSE 0 END ASC, publication_date DESC NULLS LAST, observed_at DESC, latest_version_id DESC, entry_id DESC"
        }
        ChangeFeedSort::PublishedAsc => {
            "publication_date ASC NULLS LAST, observed_at DESC, latest_version_id DESC, entry_id DESC"
        }
        ChangeFeedSort::ObservedDesc => "observed_at DESC, latest_version_id DESC, entry_id DESC",
        ChangeFeedSort::ObservedAsc => "observed_at ASC, latest_version_id DESC, entry_id DESC",
    }
}

#[derive(FromRow)]
struct FeedRow {
    entry_id: i64,
    title: String,
    region: String,
    source_category: String,
    agency: String,
    publication_date: Option<NaiveDate>,
    scheduled: bool,
    status: String,
    source_url: String,
    change_summary: Option<String>,
    observed_at: DateTime<Utc>,
}

impl FeedRow {
    fn into_item(self) -> ChangeFeedItem {
        ChangeFeedItem {
            entry_id: self.entry_id,
            title: self.title,
            region: self.region,
            source_category: self.source_category,
            agency: self.agency,
            publication_date: self.publication_date,
            scheduled: self.scheduled,
            status: self.status,
            source_url: self.source_url,
            change_summary: self.change_summary,
            changed_at: self.observed_at,
        }
    }
}

/// Failure while loading the change feed.
#[derive(Debug)]
pub enum FeedError {
    Database(sqlx::Error),
}

impl IntoResponse for FeedError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Database(error) => {
                eprintln!("change-feed query failed: {error:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use policy_shared::ChangeFeedSort;

    use super::order_by;

    #[test]
    fn default_sort_deprioritizes_scheduled_entries_and_keeps_other_orders_unchanged() {
        assert_eq!(
            order_by(ChangeFeedSort::PublishedDesc),
            "CASE WHEN publication_date > CURRENT_DATE THEN 1 ELSE 0 END ASC, publication_date DESC NULLS LAST, observed_at DESC, latest_version_id DESC, entry_id DESC"
        );
        assert_eq!(
            order_by(ChangeFeedSort::PublishedAsc),
            "publication_date ASC NULLS LAST, observed_at DESC, latest_version_id DESC, entry_id DESC"
        );
        assert_eq!(
            order_by(ChangeFeedSort::ObservedDesc),
            "observed_at DESC, latest_version_id DESC, entry_id DESC"
        );
        assert_eq!(
            order_by(ChangeFeedSort::ObservedAsc),
            "observed_at ASC, latest_version_id DESC, entry_id DESC"
        );
    }
}
