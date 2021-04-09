use anyhow::{Context, Result};
use bip39::{Mnemonic, Seed};
use ibc::core::crypto::PublicKey;
use k256::ecdsa::{SigningKey, VerifyingKey};
use tiny_hderive::bip32::ExtendedPrivKey;

const HD_PATH: &str = "m/44'/118'/0'/0/0";

pub trait Crypto {
    fn to_signing_key(&self) -> Result<SigningKey>;

    fn to_verifying_key(&self) -> Result<VerifyingKey> {
        self.to_signing_key().map(|key| key.verify_key())
    }

    fn to_public_key(&self) -> Result<PublicKey> {
        let verifying_key = self.to_verifying_key()?;
        Ok(verifying_key.into())
    }

    fn address(&self) -> Result<String> {
        let public_key = self.to_public_key()?;
        public_key.address()
    }

    fn account_address(&self, prefix: &str) -> Result<String> {
        let public_key = self.to_public_key()?;
        public_key.account_address(prefix)
    }
}

impl Crypto for Mnemonic {
    fn to_signing_key(&self) -> Result<SigningKey> {
        let seed = Seed::new(self, "");
        let private_key = ExtendedPrivKey::derive(seed.as_bytes(), HD_PATH).unwrap();

        SigningKey::from_bytes(&private_key.secret())
            .context("unable to compute signing key from verifying key")
    }
}
