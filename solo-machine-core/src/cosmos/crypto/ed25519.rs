use anyhow::{Context, Error};
use ed25519_dalek::VerifyingKey;

use ibc_proto::cosmos::crypto::ed25519::PubKey as Ed25519PubKey;

pub const ED25519_PUB_KEY_TYPE_URL: &str = "/cosmos.crypto.ed25519.PubKey";

pub fn from_verifying_key(key: &VerifyingKey) -> Ed25519PubKey {
    Ed25519PubKey {
        key: key.to_bytes().to_vec(),
    }
}

pub fn try_from_pub_key(key: &Ed25519PubKey) -> Result<VerifyingKey, Error> {
    let mut bytes = [0; 32];

    if key.key.len() != bytes.len() {
        return Err(anyhow::anyhow!(
            "invalid ed25519 public key length: {}",
            key.key.len()
        ));
    }

    bytes.copy_from_slice(&key.key);

    VerifyingKey::from_bytes(&bytes).context("unable to parse ed25519 public key from bytes")
}

impl_any_conversion!(Ed25519PubKey, ED25519_PUB_KEY_TYPE_URL);
