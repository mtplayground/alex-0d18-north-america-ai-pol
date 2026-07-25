//! Normalization for Canadian government source documents.

use chrono::NaiveDate;
use policy_shared::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, Region, Source, SourceDocument,
};
use scraper::{Html, Selector};
use serde_json::{json, Value};

/// Parses Canadian government JSON feeds and HTML publications.
pub struct CanadaGovernmentNormalizer;

impl PolicyNormalizer for CanadaGovernmentNormalizer {
    fn supports(&self, source: &Source) -> bool {
        source.region == Region::Canada
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
    let records = ["records", "items", "results"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array).cloned())
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
    let source_url = string_value(record, &["url", "link", "html_url", "source_url"])
        .unwrap_or_else(|| fallback_url.to_owned());
    let source_external_id =
        string_value(record, &["id", "uuid", "identifier", "document_number"])
            .unwrap_or_else(|| source_url.clone());
    let agency = string_value(record, &["department", "organization", "agency"])
        .unwrap_or_else(|| source.agency.clone());
    let status = string_value(record, &["status", "type", "category"])
        .unwrap_or_else(|| "published".to_owned());
    let publication_date = string_value(
        record,
        &["date_published", "publication_date", "date", "date_modified"],
    )
    .and_then(|date| parse_date(&date));
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
        region: Region::Canada,
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
    let title_selector = Selector::parse("h1, h2, h3")
        .map_err(|error| NormalizationError::new(format!("invalid title selector: {error}")))?;
    let Some(title) = html
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<String>().trim().to_owned())
        .filter(|title| !title.is_empty())
    else {
        return Ok(Vec::new());
    };
    let date_selector = Selector::parse("time[datetime]")
        .map_err(|error| NormalizationError::new(format!("invalid date selector: {error}")))?;
    let publication_date = html
        .select(&date_selector)
        .next()
        .and_then(|element| element.value().attr("datetime"))
        .and_then(parse_date);
    let canonical_content = json!({
        "title": title,
        "agency": source.agency,
        "publication_date": publication_date,
        "status": "published",
        "source_url": document.source_url,
    });

    Ok(vec![NormalizedPolicyRecord {
        source_external_id: document.source_url.to_owned(),
        title,
        region: Region::Canada,
        agency: source.agency.clone(),
        publication_date,
        status: "published".to_owned(),
        source_url: document.source_url.to_owned(),
        canonical_content,
    }])
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    value
        .get(..10)
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
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

    use super::CanadaGovernmentNormalizer;

    #[test]
    fn normalizes_a_canadian_open_data_style_record() {
        let source = Source {
            id: 3,
            region: Region::Canada,
            agency: "Treasury Board of Canada Secretariat".to_owned(),
            base_url: "https://www.canada.ca".to_owned(),
            crawl_config: json!({}),
            enabled: true,
        };
        let document = SourceDocument {
            source_url: "https://www.canada.ca/api/policies",
            content_type: "application/json",
            body: br#"{
                "items": [{
                    "uuid": "canada-ai-policy-1",
                    "title": "Directive on Automated Decision-Making",
                    "department": "Treasury Board of Canada Secretariat",
                    "date_published": "2026-07-25T12:00:00Z",
                    "status": "active",
                    "url": "https://www.canada.ca/en/government/system/digital-government.html"
                }]
            }"#,
        };

        let records = CanadaGovernmentNormalizer.normalize(&source, document).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_external_id, "canada-ai-policy-1");
        assert_eq!(records[0].region, Region::Canada);
        assert_eq!(records[0].status, "active");
        assert_eq!(records[0].publication_date.unwrap().to_string(), "2026-07-25");
    }
}
