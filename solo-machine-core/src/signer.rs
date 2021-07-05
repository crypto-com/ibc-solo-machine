//! Utilities for signing transactions
#[cfg(feature = "mnemonic-signer")]
mod mnemonic;

#[cfg(feature = "mnemonic-signer")]
pub use mnemonic::MnemonicSigner;

use anyhow::{Context, Result};
use bech32::{ToBase32, Variant};
use ripemd160::{Digest, Ripemd160};
use sha2::Sha256;

/// This trait must be implemented by all the public key providers (e.g. mnemonic, ledger, etc.)
pub trait ToPublicKey {
    /// Returns public key of signer
    fn to_public_key(&self) -> Result<Vec<u8>>;

    /// Returns accounts address for this signer for given prefix
    fn to_account_address(&self, prefix: &str) -> Result<String> {
        let public_key = self.to_public_key()?;
        let address_bytes = Ripemd160::digest(&Sha256::digest(&public_key)).to_vec();

        bech32::encode(prefix, address_bytes.to_base32(), Variant::Bech32)
            .context("unable to encode address into bech32")
    }
}

impl<T: ToPublicKey> ToPublicKey for &T {
    fn to_public_key(&self) -> Result<Vec<u8>> {
        (*self).to_public_key()
    }

    fn to_account_address(&self, prefix: &str) -> Result<String> {
        (*self).to_account_address(prefix)
    }
}

/// This trait must be implemented by all the transaction signers (e.g. mnemonic, ledger, etc.)
pub trait Signer: ToPublicKey {
    /// Signs the given message
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>>;
}

impl<T: Signer> Signer for &T {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        (*self).sign(message)
    }
}
