tonic::include_proto!("bank");

use std::str::FromStr;

use anyhow::{anyhow, ensure, Context, Error};
use bip39::{Language, Mnemonic};
use ibc::core::ics24_host::path::Path;
use rust_decimal::Decimal;
use sled::{
    transaction::{ConflictableTransactionError, TransactionError},
    IVec, Tree,
};
use tonic::{Request, Response, Status};

use crate::crypto::Crypto;

use self::bank_server::Bank;

#[derive(Clone)]
pub struct BankService {
    tree: Tree,
}

impl BankService {
    /// Creates a new instance of bank service
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn mint(
        &self,
        mnemonic: &Mnemonic,
        account_prefix: &str,
        amount: Decimal,
        denom: &str,
    ) -> Result<(), Error> {
        ensure!(
            amount.is_sign_positive(),
            "minting negative amount: {}",
            amount
        );

        let account_address = mnemonic
            .account_address(account_prefix)
            .context("unable to compute account address from mnemonic")?;

        self.mint_to(&account_address, amount, denom)
    }

    pub fn mint_to(
        &self,
        account_address: &str,
        amount: Decimal,
        denom: &str,
    ) -> Result<(), Error> {
        let path = Path::from_str(&format!("balances/{}/{}", account_address, denom))
            .context("unable to generate storage path from account prefix and denom")?;

        let response: Result<(), TransactionError<Error>> = self.tree.transaction(|tree| {
            let current_balance =
                deserialize_balance(&tree.get(&path)?).map_err(into_transaction_error)?;
            let new_balance = current_balance + amount;

            tree.insert(path.as_ref(), &new_balance.serialize())?;

            Ok(())
        });

        match response {
            Ok(_) => {
                log::info!(
                    "successfully minted {} {} for {}",
                    amount,
                    denom,
                    account_address,
                );
                Ok(())
            }
            Err(TransactionError::Storage(err)) => {
                Err(Error::from(err).context("storage error while executing transaction"))
            }
            Err(TransactionError::Abort(err)) => {
                Err(err.context("abort error while executing transaction"))
            }
        }
    }

    pub fn burn(
        &self,
        mnemonic: &Mnemonic,
        account_prefix: &str,
        amount: Decimal,
        denom: &str,
    ) -> Result<(), Error> {
        ensure!(
            amount.is_sign_positive(),
            "minting negative amount: {}",
            amount
        );

        let (path, account_address) = get_path(mnemonic, account_prefix, denom)?;

        let response: Result<(), TransactionError<Error>> = self.tree.transaction(|tree| {
            let current_balance =
                deserialize_balance(&tree.get(&path)?).map_err(into_transaction_error)?;

            if current_balance < amount {
                return Err(ConflictableTransactionError::Abort(anyhow!(
                    "insufficient balance: {}",
                    current_balance
                )));
            }

            let new_balance = current_balance - amount;

            tree.insert(path.as_ref(), &new_balance.serialize())?;

            Ok(())
        });

        match response {
            Ok(_) => {
                log::info!(
                    "successfully burnt {} {} for {}",
                    amount,
                    denom,
                    account_address,
                );
                Ok(())
            }
            Err(TransactionError::Storage(err)) => {
                Err(Error::from(err).context("storage error while executing transaction"))
            }
            Err(TransactionError::Abort(err)) => {
                Err(err.context("abort error while executing transaction"))
            }
        }
    }

    fn query_balance(
        &self,
        mnemonic: &Mnemonic,
        account_prefix: &str,
        denom: &str,
    ) -> Result<Decimal, Error> {
        let (path, _) = get_path(mnemonic, account_prefix, denom)?;
        let bytes = self.tree.get(&path)?;
        deserialize_balance(&bytes)
    }
}

fn get_path(
    mnemonic: &Mnemonic,
    account_prefix: &str,
    denom: &str,
) -> Result<(Path, String), Error> {
    let account_address = mnemonic
        .account_address(account_prefix)
        .context("unable to compute account address from mnemonic")?;

    Path::from_str(&format!("balances/{}/{}", account_address, denom))
        .context("unable to generate storage path from account prefix and denom")
        .map(|path| (path, account_address))
}

fn deserialize_balance(serialized: &Option<IVec>) -> Result<Decimal, Error> {
    match serialized {
        Some(ref serialized) => deserialize_decimal(serialized),
        None => Ok(Default::default()),
    }
}

fn deserialize_decimal(bytes: &[u8]) -> Result<Decimal, Error> {
    ensure!(bytes.len() == 16, "decimal value is not 16 bytes");

    let mut fixed_bytes = [0; 16];
    fixed_bytes.copy_from_slice(bytes);

    Ok(Decimal::deserialize(fixed_bytes))
}

fn into_transaction_error<T: Into<anyhow::Error>>(
    err: T,
) -> ConflictableTransactionError<anyhow::Error> {
    ConflictableTransactionError::Abort(err.into())
}

#[tonic::async_trait]
impl Bank for BankService {
    async fn mint(&self, request: Request<MintRequest>) -> Result<Response<MintResponse>, Status> {
        let request = request.into_inner();

        let amount = request
            .amount
            .parse()
            .context("unable to parse amount")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let denom = request.denom;
        let mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .context("unable to parse mnemonic")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let account_prefix = request.account_prefix;

        self.mint(&mnemonic, &account_prefix, amount, &denom)
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(Default::default()))
    }

    async fn burn(&self, request: Request<BurnRequest>) -> Result<Response<BurnResponse>, Status> {
        let request = request.into_inner();

        let amount = request
            .amount
            .parse()
            .context("unable to parse amount")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let denom = request.denom;
        let mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .context("unable to parse mnemonic")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let account_prefix = request.account_prefix;

        self.burn(&mnemonic, &account_prefix, amount, &denom)
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(Default::default()))
    }

    async fn query_balance(
        &self,
        request: Request<QueryBalanceRequest>,
    ) -> Result<Response<QueryBalanceResponse>, Status> {
        let request = request.into_inner();

        let denom = request.denom;
        let mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .context("unable to parse mnemonic")
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let account_prefix = request.account_prefix;

        let balance = self
            .query_balance(&mnemonic, &account_prefix, &denom)
            .map_err(|err| Status::internal(err.to_string()))?
            .to_string();

        let reply = QueryBalanceResponse { balance, denom };

        Ok(Response::new(reply))
    }
}
