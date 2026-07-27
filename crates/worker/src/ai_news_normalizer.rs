//! Normalization for RSS, Atom, and JSON AI-news feeds.

use chrono::{DateTime, NaiveDate};
use policy_shared::{
    NormalizationError, NormalizedPolicyRecord, PolicyNormalizer, Source, SourceCategory,
    SourceDocument,
};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use reqwest::Url;
use serde_json::{json, Value};

/// Parses article-oriented documents from sources categorized as AI news.
pub struct AiNewsNormalizer;

impl PolicyNormalizer for AiNewsNormalizer {
    fn supports(&self, source: &Source) -> bool {
        source.category == SourceCategory::News
    }

    fn normalize(
        &self,
        source: &Source,
        document: SourceDocument<'_>,
    ) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
        if document.content_type.to_ascii_lowercase().contains("json") {
            return normalize_json(source, document);
        }

        normalize_xml(source, document)
    }
}

#[derive(Default)]
struct XmlArticle {
    title: Option<String>,
    identifier: Option<String>,
    link: Option<String>,
    publication_date: Option<String>,
    source_name: Option<String>,
}

fn normalize_xml(
    source: &Source,
    document: SourceDocument<'_>,
) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
    let (feed_name, articles) = parse_xml_articles(document.body)?;

    Ok(articles
        .into_iter()
        .filter_map(|article| {
            normalize_article(
                source,
                document.source_url,
                feed_name.as_deref(),
                article.title,
                article.identifier,
                article.link,
                article.publication_date,
                article.source_name,
                None,
            )
        })
        .collect())
}

fn normalize_json(
    source: &Source,
    document: SourceDocument<'_>,
) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError> {
    let payload: Value = serde_json::from_slice(document.body)
        .map_err(|error| NormalizationError::new(format!("invalid news JSON document: {error}")))?;

    Ok(json_articles(&payload)
        .into_iter()
        .filter_map(|article| {
            let title = string_field(article, &["title", "headline", "name"]);
            let identifier = string_field(article, &["guid", "id", "uuid"]);
            let link = string_field(article, &["url", "link", "article_url", "web_url"]);
            let publication_date = string_field(
                article,
                &[
                    "published_at",
                    "publishedAt",
                    "publication_date",
                    "pubDate",
                    "date_published",
                    "date",
                ],
            );
            let source_name = article_source_name(article);
            let description = string_field(article, &["summary", "description", "content", "excerpt"]);

            normalize_article(
                source,
                document.source_url,
                None,
                title,
                identifier,
                link,
                publication_date,
                source_name,
                description,
            )
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn normalize_article(
    source: &Source,
    document_url: &str,
    feed_name: Option<&str>,
    title: Option<String>,
    identifier: Option<String>,
    link: Option<String>,
    publication_date: Option<String>,
    article_source_name: Option<String>,
    description: Option<String>,
) -> Option<NormalizedPolicyRecord> {
    let title = non_empty(title)?;
    let identifier = non_empty(identifier);
    let link = non_empty(link);
    let source_url = link
        .as_deref()
        .and_then(|value| resolve_article_url(document_url, value))
        .or_else(|| {
            identifier
                .as_deref()
                .and_then(|value| resolve_article_url(document_url, value))
        })?;
    let source_external_id = identifier.unwrap_or_else(|| source_url.clone());
    let agency = non_empty(article_source_name)
        .or_else(|| feed_name.map(ToOwned::to_owned).and_then(|value| non_empty(Some(value))))
        .unwrap_or_else(|| source.agency.clone());
    let publication_date = publication_date.as_deref().and_then(parse_publication_date);
    let canonical_content = json!({
        "title": title,
        "source_name": agency,
        "publication_date": publication_date,
        "source_url": source_url,
        "description": non_empty(description),
    });

    Some(NormalizedPolicyRecord {
        source_external_id,
        title,
        region: source.region,
        agency,
        publication_date,
        status: "published".to_owned(),
        source_url,
        canonical_content,
    })
}

fn parse_xml_articles(
    body: &[u8],
) -> Result<(Option<String>, Vec<XmlArticle>), NormalizationError> {
    let mut reader = Reader::from_reader(body);
    reader.trim_text(true);
    let mut buffer = Vec::new();
    let mut articles = Vec::new();
    let mut article = None;
    let mut current_tag = None;
    let mut feed_name = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let tag = tag_name(element.name().as_ref());
                if is_article_tag(&tag) {
                    article = Some(XmlArticle::default());
                    current_tag = None;
                } else {
                    if let Some(current_article) = article.as_mut() {
                        if tag == "link" {
                            if let Some(link) = article_link(&element)? {
                                current_article.link = Some(link);
                            }
                        }
                    }
                    current_tag = Some(tag);
                }
            }
            Ok(Event::Empty(element)) => {
                let tag = tag_name(element.name().as_ref());
                if tag == "link" {
                    if let Some(current_article) = article.as_mut() {
                        if let Some(link) = article_link(&element)? {
                            current_article.link = Some(link);
                        }
                    }
                }
            }
            Ok(Event::Text(text)) => {
                let text = text
                    .unescape()
                    .map_err(|error| {
                        NormalizationError::new(format!("invalid escaped XML text: {error}"))
                    })?
                    .into_owned();
                apply_xml_text(
                    article.as_mut(),
                    current_tag.as_deref(),
                    &mut feed_name,
                    text,
                );
            }
            Ok(Event::CData(text)) => {
                let text = String::from_utf8_lossy(text.as_ref()).into_owned();
                apply_xml_text(
                    article.as_mut(),
                    current_tag.as_deref(),
                    &mut feed_name,
                    text,
                );
            }
            Ok(Event::End(element)) => {
                let tag = tag_name(element.name().as_ref());
                if is_article_tag(&tag) {
                    if let Some(completed_article) = article.take() {
                        articles.push(completed_article);
                    }
                }
                current_tag = None;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(NormalizationError::new(format!(
                    "invalid RSS or Atom XML document: {error}"
                )));
            }
        }
        buffer.clear();
    }

    Ok((feed_name, articles))
}

fn apply_xml_text(
    article: Option<&mut XmlArticle>,
    current_tag: Option<&str>,
    feed_name: &mut Option<String>,
    text: String,
) {
    let Some(text) = non_empty(Some(text)) else {
        return;
    };

    match (article, current_tag) {
        (Some(article), Some("title")) => article.title = Some(text),
        (Some(article), Some("guid" | "id")) => {
            if article.identifier.is_none() {
                article.identifier = Some(text);
            }
        }
        (Some(article), Some("link")) => article.link = Some(text),
        (Some(article), Some("pubdate" | "published" | "updated" | "date")) => {
            if article.publication_date.is_none() {
                article.publication_date = Some(text);
            }
        }
        (Some(article), Some("source" | "publisher")) => article.source_name = Some(text),
        (None, Some("title")) if feed_name.is_none() => *feed_name = Some(text),
        _ => {}
    }
}

fn attribute_value(
    element: &BytesStart<'_>,
    expected_name: &str,
) -> Result<Option<String>, NormalizationError> {
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute.map_err(|error| {
            NormalizationError::new(format!("invalid XML attribute: {error}"))
        })?;
        if tag_name(attribute.key.as_ref()) == expected_name {
            return attribute
                .unescape_value()
                .map(|value| Some(value.into_owned()))
                .map_err(|error| {
                    NormalizationError::new(format!("invalid escaped XML attribute: {error}"))
                });
        }
    }

    Ok(None)
}

fn article_link(element: &BytesStart<'_>) -> Result<Option<String>, NormalizationError> {
    let relation = attribute_value(element, "rel")?;
    if relation
        .as_deref()
        .is_some_and(|relation| !relation.eq_ignore_ascii_case("alternate"))
    {
        return Ok(None);
    }

    attribute_value(element, "href")
}

fn is_article_tag(tag: &str) -> bool {
    matches!(tag, "item" | "entry")
}

fn tag_name(value: &[u8]) -> String {
    String::from_utf8_lossy(value)
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn json_articles(payload: &Value) -> Vec<&Value> {
    if let Some(articles) = payload.as_array() {
        return articles.iter().collect();
    }

    ["articles", "items", "results", "data"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array))
        .map_or_else(Vec::new, |articles| articles.iter().collect())
}

fn string_field(article: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| article.get(*name).and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .and_then(|value| non_empty(Some(value)))
}

fn article_source_name(article: &Value) -> Option<String> {
    string_field(article, &["source_name", "publisher", "publication"])
        .or_else(|| article.get("source").and_then(Value::as_str).map(ToOwned::to_owned))
        .or_else(|| {
            article
                .get("source")
                .and_then(|source| source.get("name").or_else(|| source.get("title")))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .and_then(|value| non_empty(Some(value)))
}

fn resolve_article_url(document_url: &str, candidate: &str) -> Option<String> {
    Url::parse(candidate)
        .or_else(|_| Url::parse(document_url).and_then(|base| base.join(candidate)))
        .ok()
        .map(|url| url.to_string())
}

fn parse_publication_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            value
                .get(..10)
                .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        })
        .or_else(|| DateTime::parse_from_rfc3339(value).ok().map(|date| date.date_naive()))
        .or_else(|| DateTime::parse_from_rfc2822(value).ok().map(|date| date.date_naive()))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use policy_shared::{PolicyNormalizer, Region, Source, SourceCategory, SourceDocument};
    use serde_json::json;

    use super::AiNewsNormalizer;

    fn news_source() -> Source {
        Source {
            id: 7,
            region: Region::Global,
            category: SourceCategory::News,
            agency: "Configured AI news source".to_owned(),
            base_url: "https://news.example.test".to_owned(),
            crawl_config: json!({}),
            enabled: true,
        }
    }

    #[test]
    fn rss_normalization_emits_one_record_per_article_with_guid_identity() {
        let document = SourceDocument {
            source_url: "https://news.example.test/rss.xml",
            content_type: "application/rss+xml",
            body: br#"<?xml version="1.0"?><rss><channel><title>AI Wire</title>
                <item><title>First model launch</title><guid>guid-1</guid><link>/articles/first</link><pubDate>Fri, 25 Jul 2026 12:00:00 +0000</pubDate></item>
                <item><title>Second model launch</title><guid>guid-2</guid><link>https://news.example.test/articles/second</link><pubDate>2026-07-26</pubDate></item>
            </channel></rss>"#,
        };
        let records = match AiNewsNormalizer.normalize(&news_source(), document) {
            Ok(records) => records,
            Err(error) => panic!("RSS should normalize: {error}"),
        };

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].source_external_id, "guid-1");
        assert_eq!(records[0].source_url, "https://news.example.test/articles/first");
        assert_eq!(records[0].agency, "AI Wire");
        assert_eq!(records[1].source_external_id, "guid-2");
    }

    #[test]
    fn atom_normalization_uses_atom_id_and_link_href() {
        let document = SourceDocument {
            source_url: "https://news.example.test/atom.xml",
            content_type: "application/atom+xml",
            body: br#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Atom AI</title>
                <entry><title>Atom article</title><id>tag:example.test,2026:atom-1</id><link rel="self" href="/atom.xml"/><link rel="alternate" href="/atom/article"/><published>2026-07-27T08:30:00Z</published></entry>
            </feed>"#,
        };
        let records = match AiNewsNormalizer.normalize(&news_source(), document) {
            Ok(records) => records,
            Err(error) => panic!("Atom should normalize: {error}"),
        };

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_external_id, "tag:example.test,2026:atom-1");
        assert_eq!(records[0].source_url, "https://news.example.test/atom/article");
        assert_eq!(records[0].publication_date.map(|date| date.to_string()), Some("2026-07-27".to_owned()));
    }

    #[test]
    fn json_normalization_handles_multiple_articles_and_source_names() {
        let document = SourceDocument {
            source_url: "https://news.example.test/feed.json",
            content_type: "application/feed+json",
            body: br#"{
                "articles": [
                    {"id": "json-1", "title": "JSON article one", "url": "/json/one", "published_at": "2026-07-25T12:00:00Z", "source": {"name": "JSON AI Daily"}},
                    {"guid": "json-2", "headline": "JSON article two", "link": "https://news.example.test/json/two", "date": "2026-07-26"}
                ]
            }"#,
        };
        let records = match AiNewsNormalizer.normalize(&news_source(), document) {
            Ok(records) => records,
            Err(error) => panic!("JSON should normalize: {error}"),
        };

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].source_external_id, "json-1");
        assert_eq!(records[0].agency, "JSON AI Daily");
        assert_eq!(records[1].source_external_id, "json-2");
        assert_eq!(records[1].agency, "Configured AI news source");
    }

    #[test]
    fn normalizer_only_supports_news_sources() {
        let mut source = news_source();
        assert!(AiNewsNormalizer.supports(&source));
        source.category = SourceCategory::Policy;
        assert!(!AiNewsNormalizer.supports(&source));
    }
}
