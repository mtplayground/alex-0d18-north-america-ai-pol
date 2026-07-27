//! Change-feed query service and HTTP handler.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use policy_shared::{ChangeFeedItem, ChangeFeedQuery, ChangeFeedResponse};
use sqlx::FromRow;

use crate::db::Database;

/// Returns the latest version of every policy entry, newest changes first.
pub async fn list_changes(
    State(database): State<Database>,
    Query(query): Query<ChangeFeedQuery>,
) -> Result<Json<ChangeFeedResponse>, FeedError> {
    let (limit, offset) = query.pagination();
    let (region, agency, status, category) = query.filters();
    let rows = sqlx::query_as::<_, FeedRow>(
        "SELECT entries.id AS entry_id, entries.title, entries.region, sources.category AS source_category, entries.agency, entries.publication_date, \
         entries.status, entries.source_url, latest.change_summary, latest.observed_at \
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
         ORDER BY latest.observed_at DESC, latest.id DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(i64::from(limit) + 1)
    .bind(i64::from(offset))
    .bind(region)
    .bind(agency)
    .bind(status)
    .bind(category)
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

#[derive(FromRow)]
struct FeedRow {
    entry_id: i64,
    title: String,
    region: String,
    source_category: String,
    agency: String,
    publication_date: Option<NaiveDate>,
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
