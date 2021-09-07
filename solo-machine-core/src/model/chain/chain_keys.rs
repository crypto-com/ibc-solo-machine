use std::convert::TryFrom;

use anyhow::{ensure, Context, Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow};

use crate::{ibc::core::ics24_host::identifier::ChainId, Db};

/// Signer's public key entry for an IBC enabled chain
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainKey {
    /// ID of key
    pub id: i64,
    /// Chain ID
    pub chain_id: ChainId,
    /// Public key of signer
    pub public_key: String,
    /// Creation time of chain key entry
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
/// Raw signer's public key entry for an IBC enabled chain
struct RawChainKey {
    /// ID of operation
    pub id: i64,
    /// Chain ID
    pub chain_id: String,
    /// Public key of signer
    pub public_key: String,
    /// Creation time of chain key entry
    pub created_at: DateTime<Utc>,
}

impl From<ChainKey> for RawChainKey {
    fn from(chain_key: ChainKey) -> Self {
        Self {
            id: chain_key.id,
            chain_id: chain_key.chain_id.to_string(),
            public_key: chain_key.public_key,
            created_at: chain_key.created_at,
        }
    }
}

impl TryFrom<RawChainKey> for ChainKey {
    type Error = Error;

    fn try_from(raw: RawChainKey) -> Result<Self, Self::Error> {
        Ok(Self {
            id: raw.id,
            chain_id: raw.chain_id.parse()?,
            public_key: raw.public_key,
            created_at: raw.created_at,
        })
    }
}

pub async fn add_chain_key<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
    public_key: &str,
) -> Result<()> {
    let rows_affected =
        sqlx::query("INSERT INTO chain_keys (chain_id, public_key) VALUES ($1, $2)")
            .bind(chain_id.to_string())
            .bind(public_key)
            .execute(executor)
            .await
            .context("unable to add new chain key")?
            .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding new chain key"
    );

    Ok(())
}

pub async fn get_chain_keys<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
    limit: u32,
    offset: u32,
) -> Result<Vec<ChainKey>> {
    let chain_keys: Vec<RawChainKey> = sqlx::query_as(
        "SELECT * FROM chain_keys WHERE chain_id = $1 ORDER BY id DESC LIMIT $2 OFFSET $3",
    )
    .bind(chain_id.to_string())
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
    .context("unable to query chain keys from database")?;

    chain_keys.into_iter().map(TryFrom::try_from).collect()
}
