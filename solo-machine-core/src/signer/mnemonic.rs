use anyhow::{Context, Result};
use bip39::{Mnemonic, Seed};
use k256::ecdsa::{signature::Signer as _, Signature, SigningKey};
use tiny_hderive::bip32::ExtendedPrivKey;

use crate::{Signer, ToPublicKey};

#[derive(Debug, Clone)]
/// Signer implementation using mnemonic
pub struct MnemonicSigner {
    /// Mnemonic of signer
    pub mnemonic: Mnemonic,
    /// HD path of signer
    pub hd_path: String,
    /// Bech32 prefix
    pub account_prefix: String,
}

impl MnemonicSigner {
    fn get_signing_key(&self) -> Result<SigningKey> {
        let seed = Seed::new(&self.mnemonic, "");
        let private_key = ExtendedPrivKey::derive(seed.as_bytes(), self.hd_path.as_str()).unwrap();

        SigningKey::from_bytes(&private_key.secret())
            .context("unable to compute signing key from verifying key")
    }
}

impl ToPublicKey for MnemonicSigner {
    fn to_public_key(&self) -> Result<Vec<u8>> {
        let signing_key = self.get_signing_key()?;
        let verifying_key = signing_key.verifying_key();
        Ok(verifying_key.to_bytes().to_vec())
    }

    fn get_account_prefix(&self) -> &str {
        &self.account_prefix
    }
}

impl Signer for MnemonicSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let signing_key = self.get_signing_key()?;
        let signature: Signature = signing_key.sign(message);

        Ok(signature.as_ref().to_vec())
    }
}
