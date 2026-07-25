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
    /// Credentials and request defaults for generated change summaries.
    pub ai_summarizer: AiSummarizerConfig,
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
}

impl AiSummarizerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            api_key: required("AI_SUMMARIZER_API_KEY")?,
            base_url: required("AI_SUMMARIZER_BASE_URL")?,
            model: required("AI_SUMMARIZER_MODEL")?,
        })
    }
}

/// Environment configuration could not be read or parsed.
#[derive(Debug)]
pub enum ConfigError {
    Missing(&'static str),
    InvalidBoolean(std::str::ParseBoolError),
    InvalidCadence(ParseIntError),
    InvalidListenAddress(AddrParseError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(variable) => write!(formatter, "{variable} is required"),
            Self::InvalidBoolean(error) => write!(formatter, "invalid storage path-style flag: {error}"),
            Self::InvalidCadence(error) => write!(formatter, "invalid scheduler cadence: {error}"),
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
        Self::InvalidCadence(error)
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
