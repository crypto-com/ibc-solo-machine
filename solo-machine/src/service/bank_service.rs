use anyhow::{ensure, Context, Result};
use ibc::core::ics24_host::identifier::Identifier;
use sqlx::{Acquire, Transaction};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    event::notify_event,
    model::{
        bank::{account, account_operation},
        Account, AccountOperation, OperationType,
    },
    Db, DbPool, Event, Signer, ToPublicKey,
};

/// Service that implements all the bank operations on solo machine
pub struct BankService {
    db_pool: DbPool,
    notifier: Option<UnboundedSender<Event>>,
}

impl BankService {
    /// Creates a new instance of bank service
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            notifier: None,
        }
    }

    /// Creates a new instance of bank service with notifier
    pub fn new_with_notifier(db_pool: DbPool, notifier: UnboundedSender<Event>) -> Self {
        Self {
            db_pool,
            notifier: Some(notifier),
        }
    }

    /// Mint some tokens on solo machine
    pub async fn mint(
        &self,
        signer: impl Signer,
        account_prefix: &str,
        amount: u32,
        denom: Identifier,
    ) -> Result<()> {
        let address = signer.to_account_address(account_prefix)?;

        let mut transaction = self
            .db_pool
            .begin()
            .await
            .context("unable to begin database transaction")?;

        add_tokens(
            &mut transaction,
            &address,
            amount,
            &denom,
            &OperationType::Mint,
        )
        .await?;

        transaction
            .commit()
            .await
            .context("unable to commit database transaction")?;

        notify_event(
            &self.notifier,
            Event::TokensMinted {
                address,
                amount,
                denom,
            },
        )
    }

    /// Burn some tokens on solo machine
    pub async fn burn(
        &self,
        signer: impl Signer,
        account_prefix: &str,
        amount: u32,
        denom: Identifier,
    ) -> Result<()> {
        let address = signer.to_account_address(account_prefix)?;

        let mut transaction = self
            .db_pool
            .begin()
            .await
            .context("unable to begin database transaction")?;

        remove_tokens(
            &mut transaction,
            &address,
            amount,
            &denom,
            &OperationType::Burn,
        )
        .await?;

        transaction
            .commit()
            .await
            .context("unable to commit database transaction")?;

        notify_event(
            &self.notifier,
            Event::TokensBurnt {
                address,
                amount,
                denom,
            },
        )
    }

    /// Fetch details of given account
    pub async fn account(
        &self,
        signer: impl ToPublicKey,
        account_prefix: &str,
        denom: &Identifier,
    ) -> Result<Option<Account>> {
        let account_address = signer.to_account_address(account_prefix)?;
        account::get_account(&self.db_pool, &account_address, denom).await
    }

    /// Fetch balance of given denom
    pub async fn balance(
        &self,
        signer: impl ToPublicKey,
        account_prefix: &str,
        denom: &Identifier,
    ) -> Result<u32> {
        let account_address = signer.to_account_address(account_prefix)?;
        let balance = account::get_account(&self.db_pool, &account_address, denom)
            .await?
            .map(|account| account.balance)
            .unwrap_or_default();

        Ok(balance)
    }

    /// Fetches history of all operations
    pub async fn history(
        &self,
        signer: impl ToPublicKey,
        account_prefix: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<AccountOperation>> {
        let account_address = signer.to_account_address(account_prefix)?;
        account_operation::get_operations(&self.db_pool, &account_address, limit, offset).await
    }
}

/// Adds tokens in an account
pub async fn add_tokens<'e>(
    acquire: impl Acquire<'e, Database = Db>,
    address: &str,
    amount: u32,
    denom: &Identifier,
    operation_type: &OperationType,
) -> Result<()> {
    ensure!(
        operation_type.is_addition(),
        "incorrect operation type when adding tokens"
    );

    let mut executor = acquire.acquire().await?;

    account_operation::add_operation(&mut *executor, &address, denom, amount, &operation_type)
        .await?;

    let account_exists = account::get_account(&mut *executor, &address, denom)
        .await?
        .is_some();

    if account_exists {
        account::add_balance(&mut *executor, &address, denom, amount).await?;
    } else {
        account::add_account(&mut *executor, &address, denom, amount).await?;
    }

    Ok(())
}

/// Removes tokens from an account
pub async fn remove_tokens(
    transaction: &mut Transaction<'_, Db>,
    address: &str,
    amount: u32,
    denom: &Identifier,
    operation_type: &OperationType,
) -> Result<()> {
    ensure!(
        !operation_type.is_addition(),
        "incorrect operation type when removing tokens"
    );

    account_operation::add_operation(&mut *transaction, &address, denom, amount, operation_type)
        .await?;

    let account = account::get_account(&mut *transaction, &address, denom)
        .await?
        .context("account does not exist")?;

    ensure!(account.balance >= amount, "insufficient balance");
    account::subtract_balance(transaction, &address, denom, amount).await?;

    Ok(())
}
