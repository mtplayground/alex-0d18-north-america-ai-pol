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
    let rows = sqlx::query_as::<_, FeedRow>(
        "SELECT entries.title, entries.region, entries.agency, entries.publication_date, \
         entries.status, entries.source_url, latest.change_summary, latest.observed_at \
         FROM policy_entries AS entries \
         JOIN LATERAL ( \
             SELECT id, change_summary, observed_at \
             FROM policy_versions \
             WHERE policy_entry_id = entries.id \
             ORDER BY observed_at DESC, id DESC \
             LIMIT 1 \
         ) AS latest ON TRUE \
         ORDER BY latest.observed_at DESC, latest.id DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(i64::from(limit) + 1)
    .bind(i64::from(offset))
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
    title: String,
    region: String,
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
            title: self.title,
            region: self.region,
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
pub enum FeedError {
    Database(sqlx::Error),
}

impl IntoResponse for FeedError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Database(error) => {
                eprintln!("change-feed query failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
