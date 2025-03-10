//! # Mnemonic Signer
//!
//! Signs transaction using provided mnemonic.
//!
//! ## Arguments
//!
//! Mnemonic signer can take following arguments using via environment variables:
//!
//! - `SOLO_MNEMONIC`: Mnemonic phrase (required)
//! - `SOLO_HD_PATH`: HD wallet path (default: "m/44'/118'/0'/0/0")
//! - `SOLO_ACCOUNT_PREFIX`: Account prefix for generating addresses (default: "cosmos")
//! - `SOLO_ADDRESS_ALGO`: Algorithm of the key pair (default: "secp256k1") (possible values: ["secp256k1", "eth-secp256k1"])
use std::{env, str::FromStr, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bip32::{DerivationPath, ExtendedPrivateKey, Language, Mnemonic};
use k256::ecdsa::SigningKey;
use solo_machine_core::{
    cosmos::crypto::PublicKey,
    signer::{AddressAlgo, Message, SignerRegistrar},
    Signer, ToPublicKey,
};

const DEFAULT_HD_PATH: &str = "m/44'/118'/0'/0/0";
const DEFAULT_ACCOUNT_PREFIX: &str = "cosmos";
const DEFAULT_ADDRESS_ALGO: &str = "secp256k1";

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
    pub fn from_env() -> Result<Self> {
        let mnemonic_str = get_env("SOLO_MNEMONIC")?;
        let mnemonic = Mnemonic::new(mnemonic_str, Language::English)
            .map_err(|_| anyhow!("invalid mnemonic"))?;

        let hd_path = get_env("SOLO_HD_PATH").unwrap_or_else(|_| DEFAULT_HD_PATH.to_string());
        let account_prefix =
            get_env("SOLO_ACCOUNT_PREFIX").unwrap_or_else(|_| DEFAULT_ACCOUNT_PREFIX.to_string());

        let algo = get_env("SOLO_ADDRESS_ALGO")
            .unwrap_or_else(|_| DEFAULT_ADDRESS_ALGO.to_string())
            .parse()?;

        Ok(Self {
            mnemonic,
            hd_path,
            account_prefix,
            algo,
        })
    }

    fn get_signing_key(&self) -> Result<SigningKey> {
        let seed = self.mnemonic.to_seed("");
        let hd_path = DerivationPath::from_str(&self.hd_path).context("invalid HD path")?;
        let private_key =
            ExtendedPrivateKey::<SigningKey>::derive_from_path(seed.as_bytes(), &hd_path).unwrap();

        Ok(private_key.into())
    }
}

fn get_env(key: &str) -> Result<String> {
    env::var(key).context(format!(
        "`{}` environment variable is required for mnemonic signer",
        key
    ))
}

impl ToPublicKey for MnemonicSigner {
    fn to_public_key(&self) -> Result<PublicKey> {
        let signing_key = self.get_signing_key()?;
        let verifying_key = signing_key.verifying_key();

        match self.algo {
            AddressAlgo::Secp256k1 => Ok(PublicKey::Secp256k1(*verifying_key)),
            #[cfg(feature = "ethermint")]
            AddressAlgo::EthSecp256k1 => Ok(PublicKey::EthSecp256k1(*verifying_key)),
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
    async fn sign(&self, _request_id: Option<&str>, message: Message<'_>) -> Result<Vec<u8>> {
        let signing_key = self.get_signing_key()?;

        match self.algo {
            AddressAlgo::Secp256k1 => Ok(<SigningKey as k256::ecdsa::signature::Signer<
                k256::ecdsa::Signature,
            >>::sign(&signing_key, message.as_ref())
            .to_bytes()
            .to_vec()),
            #[cfg(feature = "ethermint")]
            AddressAlgo::EthSecp256k1 => {
                let (signature, recovery_id) = signing_key.sign_recoverable(message.as_ref())?;

                let mut buf = signature.to_bytes().to_vec();
                buf.push(recovery_id.to_byte());

                buf.shrink_to_fit();

                Ok(buf)
            }
        }
    }
}

#[no_mangle]
pub fn register_signer(registrar: &mut dyn SignerRegistrar) -> Result<()> {
    registrar.register(Arc::new(MnemonicSigner::from_env()?));
    Ok(())
}
