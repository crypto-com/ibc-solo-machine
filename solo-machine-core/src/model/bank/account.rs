use std::convert::TryFrom;

use anyhow::{ensure, Context, Error, Result};
use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow};

use crate::{ibc::core::ics24_host::identifier::Identifier, Db};

#[derive(Debug)]
/// Denotes an account in solo machine
pub struct Account {
    /// Address of account
    pub address: String,
    /// Denomination of account
    pub denom: Identifier,
    /// Balance of account
    pub balance: u32,
    /// Time at which the account was created
    pub created_at: DateTime<Utc>,
    /// Time at which the account was last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
/// Denotes a raw account in solo machine
struct RawAccount {
    /// Address of account
    pub address: String,
    /// Denomination of account
    pub denom: String,
    /// Balance of account
    pub balance: u32,
    /// Time at which the account was created
    pub created_at: DateTime<Utc>,
    /// Time at which the account was last updated
    pub updated_at: DateTime<Utc>,
}

impl From<Account> for RawAccount {
    fn from(account: Account) -> Self {
        Self {
            address: account.address,
            denom: account.denom.into(),
            balance: account.balance,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

impl TryFrom<RawAccount> for Account {
    type Error = Error;

    fn try_from(raw: RawAccount) -> Result<Self, Self::Error> {
        Ok(Self {
            address: raw.address,
            denom: raw.denom.parse()?,
            balance: raw.balance,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

/// Queries an account corresponding to given address and denom from database
pub async fn get_account<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    denom: &Identifier,
) -> Result<Option<Account>> {
    let raw: Option<RawAccount> =
        sqlx::query_as("SELECT * FROM accounts WHERE address = $1 AND denom = $2")
            .bind(address)
            .bind(denom.to_string())
            .fetch_optional(executor)
            .await
            .context("unable to query account from database")?;

    raw.map(TryFrom::try_from).transpose()
}

/// Adds a new account to the database
pub async fn add_account<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    denom: &Identifier,
    balance: u32,
) -> Result<()> {
    let rows_affected =
        sqlx::query("INSERT INTO accounts (address, denom, balance) VALUES ($1, $2, $3)")
            .bind(address)
            .bind(denom.to_string())
            .bind(balance)
            .execute(executor)
            .await
            .context("unable to add new account to database")?
            .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding a new account"
    );

    Ok(())
}

/// Adds balance to an account in database
pub async fn add_balance<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    denom: &Identifier,
    balance: u32,
) -> Result<u32> {
    sqlx::query_as(
        "UPDATE accounts SET balance = balance + $1, updated_at = $2 WHERE address = $3 AND denom = $4 RETURNING balance",
    )
    .bind(balance)
    .bind(Utc::now())
    .bind(address)
    .bind(denom.to_string())
    .fetch_one(executor)
    .await
    .map(|(balance,)| balance)
    .context("unable to add new account to database")
}

/// Subtracts balance to an account in database
pub async fn subtract_balance<'e>(
    executor: impl Executor<'e, Database = Db>,
    address: &str,
    denom: &Identifier,
    balance: u32,
) -> Result<u32> {
    sqlx::query_as(
        "UPDATE accounts SET balance = balance - $1, updated_at = $2 WHERE address = $3 AND denom = $4  RETURNING balance",
    )
    .bind(balance)
    .bind(Utc::now())
    .bind(address)
    .bind(denom.to_string())
    .fetch_one(executor)
    .await
    .map(|(balance,)| balance)
    .context("unable to subtract balance to an account in database")
}
