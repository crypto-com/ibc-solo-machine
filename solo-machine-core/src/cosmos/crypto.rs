//! Cosmos public key implementation
#![allow(missing_docs)]
mod ed25519;
#[cfg(feature = "ethermint")]
mod eth_secp256k1;
mod multisig;
mod secp256k1;

pub use self::multisig::MultisigPublicKey;

use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
};

use anyhow::{anyhow, ensure, Error, Result};
use bech32::{ToBase32, Variant};
use cosmos_sdk_proto::cosmos::tx::signing::v1beta1::signature_descriptor::data::Sum as SignatureData;
use k256::ecdsa::VerifyingKey;
use prost::Message;
use prost_types::Any;
use ripemd160::{Digest, Ripemd160};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Sha256;
#[cfg(feature = "ethermint")]
use sha3::Keccak256;

#[cfg(feature = "ethermint")]
use crate::proto::ethermint::crypto::v1::ethsecp256k1::PubKey as EthSecp256k1PubKey;
use crate::proto::{
    cosmos::crypto::{
        ed25519::PubKey as Ed25519PubKey, multisig::LegacyAminoPubKey,
        secp256k1::PubKey as Secp256k1PubKey,
    },
    proto_encode, AnyConvert,
};

#[cfg(feature = "ethermint")]
use self::eth_secp256k1::ETH_SECP256K1_PUB_KEY_TYPE_URL;
use self::{
    ed25519::ED25519_PUB_KEY_TYPE_URL, multisig::MULTISIG_PUB_KEY_TYPE_URL,
    secp256k1::SECP256K1_PUB_KEY_TYPE_URL,
};

#[derive(Debug, Clone, Copy)]
/// Supported public key algorithms
pub enum PublicKeyAlgo {
    /// EthSecp256k1 (ethermint)
    #[cfg(feature = "ethermint")]
    EthSecp256k1,
    /// Secp256k1 (tendermint)
    Secp256k1,
}

impl FromStr for PublicKeyAlgo {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "ethermint")]
            "eth-secp256k1" => Ok(Self::EthSecp256k1),
            "secp256k1" => Ok(Self::Secp256k1),
            _ => Err(anyhow!("invalid public key algorithm: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PublicKey {
    #[cfg(feature = "ethermint")]
    EthSecp256k1(
        #[serde(
            serialize_with = "serialize_verifying_key",
            deserialize_with = "deserialize_verifying_key"
        )]
        k256::ecdsa::VerifyingKey,
    ),
    Secp256k1(
        #[serde(
            serialize_with = "serialize_verifying_key",
            deserialize_with = "deserialize_verifying_key"
        )]
        k256::ecdsa::VerifyingKey,
    ),
    Ed25519(ed25519_dalek::PublicKey),
    Multisig(MultisigPublicKey),
}

impl PublicKey {
    pub fn encode(&self) -> String {
        match self {
            #[cfg(feature = "ethermint")]
            Self::EthSecp256k1(key) => hex::encode_upper(key.to_bytes()),
            Self::Secp256k1(key) => hex::encode_upper(key.to_bytes()),
            Self::Ed25519(key) => hex::encode_upper(key.as_bytes()),
            Self::Multisig(_) => "unsupported key type".to_string(),
        }
    }

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

                let signature = ed25519_dalek::Signature::from(sig);

                ed25519_dalek::Verifier::verify(public_key, message, &signature).map_err(Into::into)
            }
            (PublicKey::Multisig(ref public_key), SignatureData::Multi(ref signature_data)) => {
                public_key.verify_multi_signature(message, signature_data)
            }
            #[cfg(feature = "ethermint")]
            (
                PublicKey::EthSecp256k1(ref public_key),
                SignatureData::Single(ref signature_data),
            ) => {
                let signature =
                    k256::ecdsa::Signature::try_from(signature_data.signature.as_slice())?;
                k256::ecdsa::signature::Verifier::verify(public_key, message, &signature)
                    .map_err(Into::into)
            }
            _ => Err(anyhow!("invalid public key for signature type")),
        }
    }

    fn address_bytes(&self) -> Result<Vec<u8>> {
        match self {
            #[cfg(feature = "ethermint")]
            Self::EthSecp256k1(ref key) => {
                use k256::EncodedPoint;

                let encoded_point: EncodedPoint = key.into();
                let hash =
                    Keccak256::digest(&encoded_point.to_untagged_bytes().unwrap())[12..].to_vec();

                Ok(hash)
            }
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
            #[cfg(feature = "ethermint")]
            ETH_SECP256K1_PUB_KEY_TYPE_URL => {
                let public_key: EthSecp256k1PubKey =
                    EthSecp256k1PubKey::decode(value.value.as_slice())?;
                Ok(Self::EthSecp256k1(TryFrom::try_from(&public_key)?))
            }
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
            #[cfg(feature = "ethermint")]
            Self::EthSecp256k1(ref key) => {
                let public_key: EthSecp256k1PubKey = key.into();
                public_key.to_any()
            }
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

fn serialize_verifying_key<S>(key: &VerifyingKey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    hex::serialize_upper(key.to_bytes(), serializer)
}

fn deserialize_verifying_key<'de, D>(deserializer: D) -> Result<VerifyingKey, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = hex::deserialize(deserializer)?;
    VerifyingKey::from_sec1_bytes(&bytes).map_err(serde::de::Error::custom)
}
