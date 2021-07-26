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

use sqlx::{migrate::Migrator, Sqlite, SqlitePool};

/// Database type
pub type Db = Sqlite;
/// Database pool type
pub type DbPool = SqlitePool;

/// Database migrator
pub const MIGRATOR: Migrator = sqlx::migrate!();
