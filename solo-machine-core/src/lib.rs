#![deny(missing_docs, unsafe_code)]
//! IBC solo machine
#[macro_use]
pub mod proto;

pub mod cosmos;
pub mod event;
pub mod ibc;
pub mod model;
pub mod service;
pub mod signer;
pub(crate) mod transaction_builder;

#[doc(inline)]
pub use self::{
    event::Event,
    signer::{Signer, ToPublicKey},
};

use anyhow::{Context, Result};
use sqlx::migrate::{MigrateDatabase, Migrator};

#[cfg(not(feature = "postgres"))]
pub use sqlx::{Sqlite as Db, SqlitePool as DbPool};

#[cfg(feature = "postgres")]
pub use sqlx::{PgPool as DbPool, Postgres as Db};

/// Database migrator
#[cfg(not(feature = "postgres"))]
const MIGRATOR: Migrator = sqlx::migrate!("./sqlite-migrations");

/// Database migrator
#[cfg(feature = "postgres")]
const MIGRATOR: Migrator = sqlx::migrate!("./postgres-migrations");

/// Initializes database
pub async fn init_db(connection_str: &str) -> Result<()> {
    Db::create_database(connection_str)
        .await
        .context("unable to create database")
}

/// Connects to database and returns database pool
pub async fn connect_db(connection_str: &str) -> Result<DbPool> {
    DbPool::connect(connection_str)
        .await
        .context("unable to connect to database")
}

/// Runs all the migrations on database
pub async fn run_migrations(db_pool: &DbPool) -> Result<()> {
    MIGRATOR
        .run(db_pool)
        .await
        .context("unable to run migrations")
}
