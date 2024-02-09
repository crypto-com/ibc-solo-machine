use anyhow::{Context, Error};
use k256::ecdsa::VerifyingKey;

use ibc_proto::cosmos::crypto::secp256k1::PubKey as Secp256k1PubKey;

pub const SECP256K1_PUB_KEY_TYPE_URL: &str = "/cosmos.crypto.secp256k1.PubKey";

pub fn from_verifying_key(key: &VerifyingKey) -> Secp256k1PubKey {
    Secp256k1PubKey {
        key: key.to_sec1_bytes().to_vec(),
    }
}

pub fn try_from_pub_key(key: &Secp256k1PubKey) -> Result<VerifyingKey, Error> {
    VerifyingKey::from_sec1_bytes(&key.key).context("unable to parse verifying key from sec1 bytes")
}

impl_any_conversion!(Secp256k1PubKey, SECP256K1_PUB_KEY_TYPE_URL);
