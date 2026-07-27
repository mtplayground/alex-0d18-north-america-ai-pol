//! Entry-detail query service and HTTP handler.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use policy_shared::{
    EntryDetail, EntryDetailResponse, EntryVersionDetail, PolicyChangeKind,
};
use serde_json::Value;
use sqlx::FromRow;

use crate::db::Database;

/// Returns one policy entry with its immutable version history.
pub async fn get_entry(
    Path(entry_id): Path<i64>,
    State(database): State<Database>,
) -> Result<Json<EntryDetailResponse>, DetailError> {
    let entry = sqlx::query_as::<_, EntryRow>(
        "SELECT entries.id, entries.source_id, entries.source_external_id, entries.title, entries.region, \
         sources.category AS source_category, entries.agency, entries.publication_date, entries.status, \
         entries.source_url FROM policy_entries AS entries \
         JOIN sources ON sources.id = entries.source_id WHERE entries.id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&database.pool)
    .await
    .map_err(DetailError::Database)?
    .ok_or(DetailError::NotFound)?;

    let versions = sqlx::query_as::<_, VersionRow>(
        "SELECT id, version_number, change_kind::TEXT AS change_kind, canonical_content, \
         content_hash, observed_at, change_summary \
         FROM policy_versions WHERE policy_entry_id = $1 \
         ORDER BY version_number DESC, id DESC",
    )
    .bind(entry_id)
    .fetch_all(&database.pool)
    .await
    .map_err(DetailError::Database)?
    .into_iter()
    .map(VersionRow::into_detail)
    .collect();

    Ok(Json(EntryDetailResponse {
        entry: entry.into_detail(),
        versions,
    }))
}

#[derive(FromRow)]
struct EntryRow {
    id: i64,
    source_id: i64,
    source_external_id: String,
    title: String,
    region: String,
    source_category: String,
    agency: String,
    publication_date: Option<NaiveDate>,
    status: String,
    source_url: String,
}

impl EntryRow {
    fn into_detail(self) -> EntryDetail {
        EntryDetail {
            id: self.id,
            source_id: self.source_id,
            source_external_id: self.source_external_id,
            title: self.title,
            region: self.region,
            source_category: self.source_category,
            agency: self.agency,
            publication_date: self.publication_date,
            status: self.status,
            source_url: self.source_url,
        }
    }
}

#[derive(FromRow)]
struct VersionRow {
    id: i64,
    version_number: i32,
    change_kind: String,
    canonical_content: Value,
    content_hash: String,
    observed_at: DateTime<Utc>,
    change_summary: Option<String>,
}

impl VersionRow {
    fn into_detail(self) -> EntryVersionDetail {
        EntryVersionDetail {
            id: self.id,
            version_number: self.version_number,
            change_kind: change_kind(&self.change_kind),
            canonical_content: self.canonical_content,
            content_hash: self.content_hash,
            observed_at: self.observed_at,
            change_summary: self.change_summary,
        }
    }
}

fn change_kind(value: &str) -> PolicyChangeKind {
    match value {
        "new" => PolicyChangeKind::New,
        "updated" => PolicyChangeKind::Updated,
        unexpected => {
            eprintln!("unexpected policy version change kind: {unexpected}");
            PolicyChangeKind::Updated
        }
    }
}

/// Failure while loading an entry detail response.
pub enum DetailError {
    NotFound,
    Database(sqlx::Error),
}

impl IntoResponse for DetailError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
            Self::Database(error) => {
                eprintln!("entry-detail query failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use policy_shared::PolicyChangeKind;

    use super::change_kind;

    #[test]
    fn maps_database_change_kinds_to_api_values() {
        assert_eq!(change_kind("new"), PolicyChangeKind::New);
        assert_eq!(change_kind("updated"), PolicyChangeKind::Updated);
    }
}
