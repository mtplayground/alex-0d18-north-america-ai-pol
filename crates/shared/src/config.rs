//! Environment-backed configuration shared by the backend and ingestion worker.

use std::{
    env,
    error::Error,
    fmt,
    net::{AddrParseError, SocketAddr},
    num::ParseIntError,
    time::Duration,
};

/// Configuration used by the HTTP backend.
pub struct BackendConfig {
    /// PostgreSQL connection string for the application's managed database.
    pub database_url: String,
    /// Interface and port used by the HTTP server.
    pub listen_address: SocketAddr,
    /// Directory containing the frontend's production build output.
    pub frontend_dist_dir: String,
}

impl BackendConfig {
    /// Loads backend settings from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            listen_address: optional("LISTEN_ADDRESS", "0.0.0.0:8080")?.parse()?,
            frontend_dist_dir: optional("FRONTEND_DIST_DIR", "frontend/dist")?,
        })
    }
}

/// Configuration used by the scheduled ingestion worker.
pub struct WorkerConfig {
    /// PostgreSQL connection string used to load enabled sources.
    pub database_url: String,
    /// Credentials and location for raw-source object storage.
    pub object_storage: ObjectStorageConfig,
    /// Credentials and request defaults for generated change summaries, when
    /// an AI service has been configured for this deployment.
    pub ai_summarizer: Option<AiSummarizerConfig>,
    /// Delay between ingestion passes.
    pub scheduler_cadence: Duration,
}

impl WorkerConfig {
    /// Loads worker settings from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let cadence_seconds = optional("SCHEDULER_CADENCE_SECONDS", "86400")?.parse()?;

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            object_storage: ObjectStorageConfig::from_env()?,
            ai_summarizer: AiSummarizerConfig::from_env()?,
            scheduler_cadence: Duration::from_secs(cadence_seconds),
        })
    }
}

/// S3-compatible object storage configuration.
pub struct ObjectStorageConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub prefix: String,
    pub endpoint: String,
    pub region: String,
    pub force_path_style: bool,
}

impl ObjectStorageConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            access_key_id: required("OBJECT_STORAGE_ACCESS_KEY_ID")?,
            secret_access_key: required("OBJECT_STORAGE_SECRET_ACCESS_KEY")?,
            bucket: required("OBJECT_STORAGE_BUCKET")?,
            prefix: required("OBJECT_STORAGE_PREFIX")?,
            endpoint: required("OBJECT_STORAGE_ENDPOINT")?,
            region: required("OBJECT_STORAGE_REGION")?,
            force_path_style: required("OBJECT_STORAGE_FORCE_PATH_STYLE")?.parse()?,
        })
    }
}

/// AI service configuration for policy-change summaries.
pub struct AiSummarizerConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    /// Maximum time allowed for one AI service request.
    pub request_timeout: Duration,
}

impl AiSummarizerConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        Self::from_values(
            nonempty_env("AI_SUMMARIZER_API_KEY"),
            nonempty_env("AI_SUMMARIZER_BASE_URL"),
            nonempty_env("AI_SUMMARIZER_MODEL"),
            nonempty_env("AI_SUMMARIZER_TIMEOUT_SECONDS"),
        )
    }

    fn from_values(
        api_key: Option<String>,
        base_url: Option<String>,
        model: Option<String>,
        timeout_seconds: Option<String>,
    ) -> Result<Option<Self>, ConfigError> {
        // Summaries enrich ingestion but must never stop the tracker from
        // recording a source change. Treat incomplete and blank configuration
        // as an explicitly disabled integration instead of a startup error.
        let (Some(api_key), Some(base_url), Some(model)) = (api_key, base_url, model) else {
            return Ok(None);
        };

        let timeout_seconds = timeout_seconds.unwrap_or_else(|| "20".to_owned()).parse()?;

        Ok(Some(Self {
            api_key,
            base_url,
            model,
            request_timeout: Duration::from_secs(timeout_seconds),
        }))
    }
}

/// Environment configuration could not be read or parsed.
#[derive(Debug)]
pub enum ConfigError {
    Missing(&'static str),
    InvalidBoolean(std::str::ParseBoolError),
    InvalidSeconds(ParseIntError),
    InvalidListenAddress(AddrParseError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(variable) => write!(formatter, "{variable} is required"),
            Self::InvalidBoolean(error) => write!(formatter, "invalid storage path-style flag: {error}"),
            Self::InvalidSeconds(error) => write!(formatter, "invalid duration in seconds: {error}"),
            Self::InvalidListenAddress(error) => write!(formatter, "invalid listen address: {error}"),
        }
    }
}

impl Error for ConfigError {}

impl From<std::str::ParseBoolError> for ConfigError {
    fn from(error: std::str::ParseBoolError) -> Self {
        Self::InvalidBoolean(error)
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(error: ParseIntError) -> Self {
        Self::InvalidSeconds(error)
    }
}

impl From<AddrParseError> for ConfigError {
    fn from(error: AddrParseError) -> Self {
        Self::InvalidListenAddress(error)
    }
}

fn required(variable: &'static str) -> Result<String, ConfigError> {
    env::var(variable).map_err(|_| ConfigError::Missing(variable))
}

fn optional(variable: &'static str, default: &'static str) -> Result<String, ConfigError> {
    match env::var(variable) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.to_owned()),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::Missing(variable)),
    }
}

fn nonempty_env(variable: &'static str) -> Option<String> {
    env::var(variable)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::AiSummarizerConfig;

    #[test]
    fn blank_or_incomplete_summarizer_configuration_is_disabled() {
        let blank = AiSummarizerConfig::from_values(None, None, None, None);
        assert!(matches!(blank, Ok(None)));

        let incomplete = AiSummarizerConfig::from_values(
            Some("key".to_owned()),
            None,
            Some("model".to_owned()),
            None,
        );
        assert!(matches!(incomplete, Ok(None)));
    }

    #[test]
    fn configured_summarizer_uses_default_timeout_when_blank() {
        let config = AiSummarizerConfig::from_values(
            Some("key".to_owned()),
            Some("https://ai.example.test".to_owned()),
            Some("model".to_owned()),
            None,
        );

        match config {
            Ok(Some(config)) => assert_eq!(config.request_timeout.as_secs(), 20),
            Ok(None) => panic!("complete summarizer settings should enable summaries"),
            Err(error) => panic!("complete summarizer settings should parse: {error}"),
        }
    }
}
