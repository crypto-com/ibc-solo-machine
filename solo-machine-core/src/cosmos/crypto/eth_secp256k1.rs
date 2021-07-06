use std::convert::TryFrom;

use anyhow::{Context, Error};
use k256::ecdsa::VerifyingKey;

use crate::proto::ethermint::crypto::v1alpha1::ethsecp256k1::PubKey as EthSecp256k1PubKey;

pub const ETH_SECP256K1_PUB_KEY_TYPE_URL: &str = "/ethermint.crypto.v1alpha1.ethsecp256k1.PubKey";

impl_any_conversion!(EthSecp256k1PubKey, ETH_SECP256K1_PUB_KEY_TYPE_URL);

impl From<&VerifyingKey> for EthSecp256k1PubKey {
    fn from(key: &VerifyingKey) -> Self {
        Self {
            key: key.to_bytes().to_vec(),
        }
    }
}

impl TryFrom<&EthSecp256k1PubKey> for VerifyingKey {
    type Error = Error;

    fn try_from(value: &EthSecp256k1PubKey) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(&value.key).context("unable to parse verifying key from sec1 bytes")
    }
}
