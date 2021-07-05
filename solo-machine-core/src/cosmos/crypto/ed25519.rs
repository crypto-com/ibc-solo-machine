use std::convert::TryFrom;

use anyhow::{Context, Error};
use ed25519_dalek::PublicKey;

use crate::proto::cosmos::crypto::ed25519::PubKey as Ed25519PubKey;

pub const ED25519_PUB_KEY_TYPE_URL: &str = "/cosmos.crypto.ed25519.PubKey";

impl From<&PublicKey> for Ed25519PubKey {
    fn from(key: &PublicKey) -> Self {
        Self {
            key: key.to_bytes().to_vec(),
        }
    }
}

impl TryFrom<&Ed25519PubKey> for PublicKey {
    type Error = Error;

    fn try_from(key: &Ed25519PubKey) -> Result<Self, Self::Error> {
        Self::from_bytes(&key.key).context("unable to parse ed25519 public key from bytes")
    }
}

impl_any_conversion!(Ed25519PubKey, ED25519_PUB_KEY_TYPE_URL);
