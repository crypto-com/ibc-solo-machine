use std::str::FromStr;

use anyhow::{anyhow, Context, Error, Result};
use async_trait::async_trait;
use bip32::{DerivationPath, ExtendedPrivateKey, Mnemonic};
use k256::ecdsa::{signature::DigestSigner, Signature, SigningKey};
use ripemd160::Digest;

use crate::{cosmos::crypto::PublicKey, Signer, ToPublicKey};

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

#[derive(Clone)]
/// Signer implementation using mnemonic
pub struct MnemonicSigner {
    /// Mnemonic of signer
    pub mnemonic: Mnemonic,
    /// HD path of signer
    pub hd_path: String,
    /// Bech32 prefix
    pub account_prefix: String,
    /// Algorithm used for address generation
    pub algo: AddressAlgo,
}

impl MnemonicSigner {
    fn get_signing_key(&self) -> Result<SigningKey> {
        let seed = self.mnemonic.to_seed("");
        let hd_path = DerivationPath::from_str(&self.hd_path).context("invalid HD path")?;
        let private_key =
            ExtendedPrivateKey::<SigningKey>::derive_from_path(seed.as_bytes(), &hd_path).unwrap();

        Ok(private_key.into())
    }
}

impl ToPublicKey for MnemonicSigner {
    fn to_public_key(&self) -> Result<PublicKey> {
        let signing_key = self.get_signing_key()?;
        let verifying_key = signing_key.verifying_key();

        match self.algo {
            AddressAlgo::Secp256k1 => Ok(PublicKey::Secp256k1(verifying_key)),
            #[cfg(feature = "ethermint")]
            AddressAlgo::EthSecp256k1 => Ok(PublicKey::EthSecp256k1(verifying_key)),
        }
    }

    fn get_account_prefix(&self) -> &str {
        &self.account_prefix
    }

    fn to_account_address(&self) -> Result<String> {
        self.to_public_key()?
            .account_address(self.get_account_prefix())
    }
}

#[async_trait]
impl Signer for MnemonicSigner {
    async fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let signing_key = self.get_signing_key()?;

        let signature: Signature = match self.algo {
            AddressAlgo::Secp256k1 => signing_key.sign_digest(sha2::Sha256::new().chain(message)),
            #[cfg(feature = "ethermint")]
            AddressAlgo::EthSecp256k1 => {
                signing_key.sign_digest(sha3::Keccak256::new().chain(message))
            }
        };

        Ok(signature.as_ref().to_vec())
    }
}
