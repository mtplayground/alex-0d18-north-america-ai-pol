//! Normalization for U.S. government source documents.

use chrono::NaiveDate;
use policy_shared::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, Region, Source, SourceDocument,
};
use scraper::{Html, Selector};
use serde_json::{json, Value};

/// Parses U.S. government JSON feeds and HTML publications.
pub struct UsGovernmentNormalizer;

impl PolicyNormalizer for UsGovernmentNormalizer {
    fn supports(&self, source: &Source) -> bool {
        source.region == Region::UnitedStates
    }

    fn normalize(
        &self,
        source: &Source,
        document: SourceDocument<'_>,
    ) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
        if document.content_type.contains("json") {
            return normalize_json(source, document);
        }

        normalize_html(source, document)
    }
}

fn normalize_json(
    source: &Source,
    document: SourceDocument<'_>,
) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
    let payload: Value = serde_json::from_slice(document.body).map_err(|error| {
        NormalizationError::new(format!("invalid JSON source document: {error}"))
    })?;
    let records = payload
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![payload]);

    Ok(records
        .iter()
        .filter_map(|record| normalized_json_record(source, document.source_url, record))
        .collect())
}

fn normalized_json_record(
    source: &Source,
    fallback_url: &str,
    record: &Value,
) -> Option<NormalizedPolicyRecord> {
    let title = string_value(record, &["title", "name"])?;
    let source_url = string_value(record, &["html_url", "url", "source_url"])
        .unwrap_or_else(|| fallback_url.to_owned());
    let source_external_id = string_value(record, &["document_number", "id", "slug"])
        .unwrap_or_else(|| source_url.clone());
    let agency = record
        .get("agency_names")
        .and_then(Value::as_array)
        .and_then(|agencies| agencies.first())
        .and_then(Value::as_str)
        .map_or_else(|| source.agency.clone(), ToOwned::to_owned);
    let status = string_value(record, &["status", "type", "document_type"])
        .unwrap_or_else(|| "published".to_owned());
    let publication_date =
        string_value(record, &["publication_date", "effective_date", "date"])
            .and_then(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok());
    let canonical_content = json!({
        "title": title,
        "agency": agency,
        "publication_date": publication_date,
        "status": status,
        "source_url": source_url,
    });

    Some(NormalizedPolicyRecord {
        source_external_id,
        title,
        region: Region::UnitedStates,
        agency,
        publication_date,
        status,
        source_url,
        canonical_content,
    })
}

fn normalize_html(
    source: &Source,
    document: SourceDocument<'_>,
) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
    let body = std::str::from_utf8(document.body).map_err(|error| {
        NormalizationError::new(format!("source HTML was not UTF-8: {error}"))
    })?;
    let html = Html::parse_document(body);
    let selector = Selector::parse("h1, h2, h3")
        .map_err(|error| NormalizationError::new(format!("invalid title selector: {error}")))?;
    let Some(title) = html
        .select(&selector)
        .next()
        .map(|element| element.text().collect::<String>().trim().to_owned())
        .filter(|title| !title.is_empty())
    else {
        return Ok(Vec::new());
    };
    let canonical_content = json!({
        "title": title,
        "agency": source.agency,
        "status": "published",
        "source_url": document.source_url,
    });

    Ok(vec![NormalizedPolicyRecord {
        source_external_id: document.source_url.to_owned(),
        title,
        region: Region::UnitedStates,
        agency: source.agency.clone(),
        publication_date: None,
        status: "published".to_owned(),
        source_url: document.source_url.to_owned(),
        canonical_content,
    }])
}

fn string_value(record: &Value, keys: &[&str]) -> Option<String> {
    keys
        .iter()
        .find_map(|key| record.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use policy_shared::{PolicyNormalizer, Region, Source, SourceDocument};
    use serde_json::json;

    use super::UsGovernmentNormalizer;

    #[test]
    fn normalizes_a_federal_register_style_result() {
        let source = Source {
            id: 1,
            region: Region::UnitedStates,
            agency: "Office of the Federal Register".to_owned(),
            base_url: "https://www.federalregister.gov".to_owned(),
            crawl_config: json!({}),
            enabled: true,
        };
        let document = SourceDocument {
            source_url: "https://www.federalregister.gov/documents",
            content_type: "application/json",
            body: br#"{
                "results": [{
                    "document_number": "2026-12345",
                    "title": "Example AI Policy",
                    "agency_names": ["Example Agency"],
                    "publication_date": "2026-07-25",
                    "type": "Rule",
                    "html_url": "https://www.federalregister.gov/documents/2026/12345"
                }]
            }"#,
        };

        let records = UsGovernmentNormalizer.normalize(&source, document).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_external_id, "2026-12345");
        assert_eq!(records[0].agency, "Example Agency");
        assert_eq!(records[0].status, "Rule");
        assert_eq!(records[0].publication_date.unwrap().to_string(), "2026-07-25");
    }
}
