//! Utilities for signing transactions
use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;

use crate::cosmos::crypto::PublicKey;

#[derive(Debug, Clone, Copy)]
/// Supported algorithms for address generation
pub enum AddressAlgo {
    /// Secp256k1 (tendermint)
    Secp256k1,
    #[cfg(feature = "ethermint")]
    /// EthSecp256k1 (ethermint)
    EthSecp256k1,
}

impl FromStr for AddressAlgo {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "secp256k1" => Ok(Self::Secp256k1),
            #[cfg(feature = "ethermint")]
            "eth-secp256k1" => Ok(Self::EthSecp256k1),
            _ => Err(anyhow!("invalid address generation algorithm: {}", s)),
        }
    }
}

/// This trait must be implemented by all the public key providers (e.g. mnemonic, ledger, etc.)
pub trait ToPublicKey {
    /// Returns public key of signer
    fn to_public_key(&self) -> Result<PublicKey>;

    /// Returns account prefix for computing bech32 addresses
    fn get_account_prefix(&self) -> &str;

    /// Returns accounts address for this signer for given prefix
    fn to_account_address(&self) -> Result<String>;
}

impl<T: ToPublicKey> ToPublicKey for &T {
    fn to_public_key(&self) -> Result<PublicKey> {
        (*self).to_public_key()
    }

    fn get_account_prefix(&self) -> &str {
        (*self).get_account_prefix()
    }

    fn to_account_address(&self) -> Result<String> {
        (*self).to_account_address()
    }
}

impl<T: ToPublicKey + ?Sized> ToPublicKey for Arc<T> {
    fn to_public_key(&self) -> Result<PublicKey> {
        (**self).to_public_key()
    }

    fn get_account_prefix(&self) -> &str {
        (**self).get_account_prefix()
    }

    fn to_account_address(&self) -> Result<String> {
        (**self).to_account_address()
    }
}

/// This trait must be implemented by all the transaction signers (e.g. mnemonic, ledger, etc.)
#[async_trait]
pub trait Signer: ToPublicKey + Send + Sync {
    /// Signs the given message
    async fn sign(&self, message: &[u8]) -> Result<Vec<u8>>;
}

#[async_trait]
impl<T: Signer> Signer for &T {
    async fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        (*self).sign(message).await
    }
}

#[async_trait]
impl<T: Signer + ?Sized> Signer for Arc<T> {
    async fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        (**self).sign(message).await
    }
}

/// Trait to register a signer
pub trait SignerRegistrar {
    /// Registers a new signer
    fn register(&mut self, signer: Arc<dyn Signer>);
}
