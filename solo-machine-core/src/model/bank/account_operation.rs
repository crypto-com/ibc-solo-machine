use std::{
    convert::{TryFrom, TryInto},
    fmt,
};

use anyhow::{ensure, Context, Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, Executor, FromRow};

use crate::{
    ibc::core::ics24_host::identifier::{ChainId, Identifier},
    Db,
};

/// Denotes an operation on an account
#[derive(Debug)]
pub struct AccountOperation {
    /// ID of operation
    pub id: i64,
    /// Address of the account
    pub address: String,
    /// Denom of tokens
    pub denom: Identifier,
    /// Amount of tokens
    pub amount: u32,
    /// Type of operation
    pub operation_type: OperationType,
    /// Time at which this operation was created
    pub created_at: DateTime<Utc>,
}

/// Denotes an operation on an account
#[derive(Debug, FromRow)]
pub struct RawAccountOperation {
    /// ID of operation
    pub id: i64,
    /// Address of the account
    pub address: String,
    /// Denom of tokens
    pub denom: String,
    /// Amount of tokens
    pub amount: u32,
    /// Type of operation
    pub operation_type: Json<OperationType>,
    /// Time at which this operation was created
    pub created_at: DateTime<Utc>,
}

impl From<AccountOperation> for RawAccountOperation {
    fn from(op: AccountOperation) -> Self {
        Self {
            id: op.id,
            address: op.address,
            denom: op.denom.to_string(),
            amount: op.amount,
            operation_type: Json(op.operation_type),
            created_at: op.created_at,
        }
    }
}

impl TryFrom<RawAccountOperation> for AccountOperation {
    type Error = Error;

    fn try_from(op: RawAccountOperation) -> Result<Self, Self::Error> {
        Ok(Self {
            id: op.id,
            address: op.address,
            denom: op.denom.parse()?,
            amount: op.amount,
            operation_type: op.operation_type.0,
            created_at: op.created_at,
        })
    }
}

/// Different types of possible operations on an account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    /// Mint some tokens
    Mint,
    /// Burn some tokens
    Burn,
    /// Send some tokens to IBC enabled chain
    Send {
        /// Sent to Chain ID
        chain_id: ChainId,
    },
    /// Receive some tokens from IBC enabled chain
    Receive {
        /// Received from Chain ID
        chain_id: ChainId,
    },
}

impl OperationType {
    /// Returns true of operation type denotes addition of tokens, false otherwise
    pub(crate) fn is_addition(&self) -> bool {
        matches!(self, Self::Mint | Self::Receive { .. })
    }
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mint => write!(f, "mint"),
            Self::Burn => write!(f, "burn"),
            Self::Send { chain_id } => write!(f, "send [{}]", chain_id),
            Self::Receive { chain_id } => write!(f, "receive [{}]", chain_id),
        }
    }
}

/// Adds an account operation to database
pub async fn add_operation<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    denom: &Identifier,
    amount: u32,
    operation_type: &OperationType,
) -> Result<()> {
    let operation_type = Json(operation_type);

    let rows_affected =
        sqlx::query("INSERT INTO account_operations (address, denom, amount, operation_type) VALUES ($1, $2, $3, $4)")
            .bind(address)
            .bind(denom.to_string())
            .bind(amount)
            .bind(operation_type)
            .execute(executor)
            .await
            .context("unable to add new account operation to database")?
            .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding a new account operation"
    );

    Ok(())
}

/// Fetches account operations from database
pub async fn get_operations<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<AccountOperation>> {
    let raw: Vec<RawAccountOperation> = sqlx::query_as(
        "SELECT * FROM account_operations WHERE address = $1 ORDER BY id DESC LIMIT $2 OFFSET $3",
    )
    .bind(address)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
    .context("unable to query account operations from database")?;

    raw.into_iter().map(TryInto::try_into).collect()
}
