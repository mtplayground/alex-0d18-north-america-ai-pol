//! S3-compatible raw document storage for ingestion snapshots.

use std::error::Error;

use aws_sdk_s3::{
    config::{BehaviorVersion, Credentials, Region},
    primitives::ByteStream,
    Client,
};
use chrono::{DateTime, Utc};
use policy_shared::config::ObjectStorageConfig;
use sha2::{Digest, Sha256};

/// A persisted raw document and the key stored alongside its policy version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotReference {
    /// Object-storage key for the immutable raw source document.
    pub object_key: String,
}

/// Client for the per-application Ideavibes object-storage bucket.
pub struct SnapshotStorage {
    client: Client,
    bucket: String,
    prefix: String,
}

impl SnapshotStorage {
    /// Creates a storage client using the provisioned S3-compatible credentials.
    pub fn from_config(config: &ObjectStorageConfig) -> Self {
        let credentials = Credentials::new(
            config.access_key_id.clone(),
            config.secret_access_key.clone(),
            None,
            None,
            "ideavibes-object-storage",
        );
        let sdk_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .credentials_provider(credentials)
            .endpoint_url(&config.endpoint)
            .force_path_style(config.force_path_style)
            .build();

        Self {
            client: Client::from_conf(sdk_config),
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        }
    }

    /// Uploads an immutable raw source document and returns its storage reference.
    pub async fn store(
        &self,
        source_id: i64,
        source_url: &str,
        content_hash: &str,
        content_type: &str,
        document: Vec<u8>,
    ) -> Result<SnapshotReference, Box<dyn Error + Send + Sync>> {
        let object_key = snapshot_key(
            &self.prefix,
            source_id,
            source_url,
            content_hash,
            Utc::now(),
        );
        let content_length = i64::try_from(document.len())?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .content_type(content_type)
            .content_length(content_length)
            .body(ByteStream::from(document))
            .send()
            .await?;

        Ok(SnapshotReference { object_key })
    }
}

fn snapshot_key(
    prefix: &str,
    source_id: i64,
    source_url: &str,
    content_hash: &str,
    observed_at: DateTime<Utc>,
) -> String {
    let prefix = prefix.trim_matches('/');
    let date = observed_at.format("%Y/%m/%d");
    let url_hash = format!("{:x}", Sha256::digest(source_url.as_bytes()));

    if prefix.is_empty() {
        format!("sources/{source_id}/{date}/{url_hash}/{content_hash}")
    } else {
        format!("{prefix}/sources/{source_id}/{date}/{url_hash}/{content_hash}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::snapshot_key;

    #[test]
    fn snapshot_key_is_partitioned_by_source_date_and_content() {
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 25, 12, 0, 0).unwrap();

        assert_eq!(
            snapshot_key(
                "raw-snapshots/",
                42,
                "https://example.test/feed.xml",
                "aabbcc",
                observed_at,
            ),
            "raw-snapshots/sources/42/2026/07/25/59631129751a97ebca34dfaa8b87cdbe7baf8aa70c7d083627d47e4f6afcbb23/aabbcc"
        );
    }
}
