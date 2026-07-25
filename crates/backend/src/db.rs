//! PostgreSQL connection and migration support.

use std::{env, error::Error, fmt};

use sqlx::{
    migrate::MigrateError,
    postgres::{PgPool, PgPoolOptions},
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// Shared PostgreSQL access for the HTTP application.
#[derive(Clone)]
pub struct Database {
    pub(crate) pool: PgPool,
}

impl Database {
    /// Opens the managed PostgreSQL connection described by `DATABASE_URL`.
    pub async fn connect_from_env() -> Result<Self, DatabaseError> {
        let database_url = env::var("DATABASE_URL").map_err(|_| DatabaseError::MissingUrl)?;
        let pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;

        let database = Self { pool };
        MIGRATOR.run(&database.pool).await?;

        Ok(database)
    }
}

/// Errors that can occur while establishing the application's database layer.
#[derive(Debug)]
pub enum DatabaseError {
    /// The deployment did not provide the managed PostgreSQL connection string.
    MissingUrl,
    /// SQLx could not connect to PostgreSQL.
    Sqlx(sqlx::Error),
    /// SQLx could not apply an embedded migration.
    Migration(MigrateError),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUrl => formatter.write_str("DATABASE_URL is required"),
            Self::Sqlx(error) => write!(formatter, "database connection failed: {error}"),
            Self::Migration(error) => write!(formatter, "database migration failed: {error}"),
        }
    }
}

impl Error for DatabaseError {}

impl From<sqlx::Error> for DatabaseError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<MigrateError> for DatabaseError {
    fn from(error: MigrateError) -> Self {
        Self::Migration(error)
    }
}
