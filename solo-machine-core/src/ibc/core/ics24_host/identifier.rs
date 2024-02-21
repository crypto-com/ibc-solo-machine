use core::fmt;
use std::{convert::TryFrom, ops::Deref, str::FromStr};

use anyhow::{ensure, Error};
use ibc_proto::ibc::core::commitment::v1::MerklePrefix;
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ibc::core::ics02_client::client_type::ClientType;

pub(crate) const MAX_IDENTIFIER_LEN: usize = 64;
const VALID_CHAIN_ID_PATTERN: &str = r"^.+[^-]-{1}[1-9][0-9]*$";
const VALID_ID_PATTERN: &str = r"^[a-zA-Z0-9\._\+\-\#\[\]<>]+$";

macro_rules! impl_id {
    ($doc: expr, $name: ident, $min_len: expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name(Identifier);

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let id = s.parse::<Identifier>()?;
                id.validate_length($min_len, MAX_IDENTIFIER_LEN)?;

                Ok(Self(id))
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0.into()
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Deref for $name {
            type Target = Identifier;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

impl_id!("A client identifier", ClientId, 9);
impl_id!("A connection identifier", ConnectionId, 10);
impl_id!("A channel identifier", ChannelId, 8);
impl_id!("A port identifier", PortId, 2);

impl ClientId {
    pub fn generate(client_type: ClientType) -> ClientId {
        match client_type {
            ClientType::Tendermint => Self(Identifier::generate("07-tendermint", 4).unwrap()),
        }
    }
}

impl ConnectionId {
    pub fn generate() -> ConnectionId {
        Self(Identifier::generate("connection", 4).unwrap())
    }
}

impl ChannelId {
    pub fn generate() -> ChannelId {
        Self(Identifier::generate("channel", 4).unwrap())
    }
}

/// A chain identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainId {
    id: Identifier,
    version: u64,
}

impl ChainId {
    /// Returns version of chain id
    pub fn version(&self) -> u64 {
        self.version
    }
}

impl FromStr for ChainId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse::<Identifier>()?;
        id.validate_length(1, MAX_IDENTIFIER_LEN)?;

        let regex = Regex::new(VALID_CHAIN_ID_PATTERN).unwrap();

        let version = if regex.is_match(&id) {
            let split = id.split('-').collect::<Vec<_>>();
            split
                .last()
                .map(|version| version.parse().unwrap_or(0))
                .unwrap_or(0)
        } else {
            0
        };

        Ok(Self { id, version })
    }
}

impl AsRef<[u8]> for ChainId {
    fn as_ref(&self) -> &[u8] {
        self.id.as_ref()
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl From<ChainId> for String {
    fn from(value: ChainId) -> Self {
        value.id.into()
    }
}

impl Deref for ChainId {
    type Target = Identifier;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

/// An identifier
///
/// # Specs
///
/// <https://github.com/cosmos/ibc/tree/master/spec/core/ics-024-host-requirements#paths-identifiers-separators>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identifier(String);

impl Identifier {
    pub fn generate(prefix: &str, suffix_len: usize) -> Result<Self, Error> {
        let mut rng = rand::thread_rng();

        let suffix: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(suffix_len)
            .collect();

        format!("{}-{}", prefix, suffix).parse()
    }

    fn validate_length(&self, min: usize, max: usize) -> Result<(), Error> {
        let id_len = self.0.len();

        ensure!(
            id_len >= min && id_len <= max,
            "identifier {} has invalid length: {}, must be between {}-{} characters",
            self.0,
            id_len,
            min,
            max
        );

        Ok(())
    }
}

impl AsRef<[u8]> for Identifier {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(!s.trim().is_empty(), "identifier cannot be blank");

        ensure!(
            !s.contains('/'),
            "identifier {} cannot contain separator '/'",
            s
        );

        let regex = Regex::new(VALID_ID_PATTERN).unwrap();

        ensure!(regex.is_match(s), "identifier {} must contain only alphanumeric or the following characters: '.', '_', '+', '-', '#', '[', ']', '<', '>'", s);

        let id = Self(s.into());
        id.validate_length(1, MAX_IDENTIFIER_LEN)?;

        Ok(id)
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Identifier> for String {
    fn from(id: Identifier) -> Self {
        id.0
    }
}

impl Deref for Identifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Identifier> for MerklePrefix {
    fn from(id: Identifier) -> Self {
        MerklePrefix {
            key_prefix: id.0.into_bytes(),
        }
    }
}

impl TryFrom<&MerklePrefix> for Identifier {
    type Error = Error;

    fn try_from(value: &MerklePrefix) -> Result<Self, Self::Error> {
        std::str::from_utf8(&value.key_prefix)?.parse()
    }
}
