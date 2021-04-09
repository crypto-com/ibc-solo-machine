mod ed25519;
mod multisig;
mod secp256k1;

pub use self::multisig::MultisigPublicKey;

use std::convert::{TryFrom, TryInto};

use anyhow::{anyhow, ensure, Result};
use bech32::{ToBase32, Variant};
use cosmos_sdk_proto::cosmos::tx::signing::v1beta1::signature_descriptor::data::Sum as SignatureData;
use prost::Message;
use prost_types::Any;
use ripemd160::{Digest, Ripemd160};
use sha2::Sha256;

use crate::proto::{
    cosmos::crypto::{
        ed25519::PubKey as Ed25519PubKey, multisig::LegacyAminoPubKey,
        secp256k1::PubKey as Secp256k1PubKey,
    },
    proto_encode, AnyConvert,
};

use self::{
    ed25519::ED25519_PUB_KEY_TYPE_URL, multisig::MULTISIG_PUB_KEY_TYPE_URL,
    secp256k1::SECP256K1_PUB_KEY_TYPE_URL,
};

#[derive(Debug)]
pub enum PublicKey {
    Secp256k1(k256::ecdsa::VerifyingKey),
    Ed25519(ed25519_dalek::PublicKey),
    Multisig(MultisigPublicKey),
}

impl PublicKey {
    pub fn address(&self) -> Result<String> {
        Ok(hex::encode(&self.address_bytes()?))
    }

    pub fn account_address(&self, prefix: &str) -> Result<String> {
        bech32::encode(prefix, self.address_bytes()?.to_base32(), Variant::Bech32)
            .map_err(Into::into)
    }

    pub fn verify_signature(&self, message: &[u8], signature_data: &SignatureData) -> Result<()> {
        match (self, signature_data) {
            (PublicKey::Secp256k1(ref public_key), SignatureData::Single(ref signature_data)) => {
                let signature =
                    k256::ecdsa::Signature::try_from(signature_data.signature.as_slice())?;
                k256::ecdsa::signature::Verifier::verify(public_key, message, &signature)
                    .map_err(Into::into)
            }
            (PublicKey::Ed25519(ref public_key), SignatureData::Single(ref signature_data)) => {
                ensure!(
                    signature_data.signature.len() == 64,
                    "signature lendth should be equal to 64"
                );

                let mut sig = [0; 64];
                sig.copy_from_slice(&signature_data.signature);

                let signature = ed25519_dalek::Signature::new(sig);

                ed25519_dalek::Verifier::verify(public_key, message, &signature).map_err(Into::into)
            }
            (PublicKey::Multisig(ref public_key), SignatureData::Multi(ref signature_data)) => {
                public_key.verify_multi_signature(message, signature_data)
            }
            _ => Err(anyhow!("invalid public key for signature type")),
        }
    }

    fn address_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::Secp256k1(ref key) => {
                Ok(Ripemd160::digest(&Sha256::digest(&key.to_bytes())).to_vec())
            }
            Self::Ed25519(ref key) => Ok(Sha256::digest(key.as_bytes()).to_vec()),
            Self::Multisig(ref key) => {
                let multisig_key: LegacyAminoPubKey = key.try_into()?;
                let bytes = Sha256::digest(&proto_encode(&multisig_key)?);
                Ok(bytes[..20].to_vec())
            }
        }
    }
}

impl From<k256::ecdsa::VerifyingKey> for PublicKey {
    fn from(key: k256::ecdsa::VerifyingKey) -> Self {
        PublicKey::Secp256k1(key)
    }
}

impl AnyConvert for PublicKey {
    fn from_any(value: &Any) -> Result<Self> {
        match value.type_url.as_str() {
            SECP256K1_PUB_KEY_TYPE_URL => {
                let public_key: Secp256k1PubKey = Secp256k1PubKey::decode(value.value.as_slice())?;
                Ok(Self::Secp256k1(TryFrom::try_from(&public_key)?))
            }
            ED25519_PUB_KEY_TYPE_URL => {
                let public_key: Ed25519PubKey = Ed25519PubKey::decode(value.value.as_slice())?;
                Ok(Self::Ed25519(TryFrom::try_from(&public_key)?))
            }
            MULTISIG_PUB_KEY_TYPE_URL => {
                let multisig_key: LegacyAminoPubKey =
                    LegacyAminoPubKey::decode(value.value.as_slice())?;
                Ok(Self::Multisig(TryFrom::try_from(&multisig_key)?))
            }
            other => Err(anyhow!("unknown type url for `Any` type: `{}`", other)),
        }
    }

    fn to_any(&self) -> Result<Any> {
        match self {
            Self::Secp256k1(ref key) => {
                let public_key: Secp256k1PubKey = key.into();
                public_key.to_any()
            }
            Self::Ed25519(ref key) => {
                let public_key: Ed25519PubKey = key.into();
                public_key.to_any()
            }
            Self::Multisig(ref key) => {
                let multisig_key: LegacyAminoPubKey = key.try_into()?;
                multisig_key.to_any()
            }
        }
    }
}
