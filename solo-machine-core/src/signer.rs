//! Utilities for signing transactions
#[cfg(feature = "mnemonic-signer")]
mod mnemonic;

#[cfg(feature = "mnemonic-signer")]
pub use mnemonic::{AddressAlgo, MnemonicSigner};

use anyhow::Result;
use async_trait::async_trait;

use crate::cosmos::crypto::PublicKey;

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
