use std::{
    convert::{TryFrom, TryInto},
    fmt,
    ops::Deref,
    str::FromStr,
};

use anyhow::{ensure, Error};
use cosmos_sdk_proto::ibc::core::{client::v1::Height, commitment::v1::MerklePath};

use crate::core::ics02_client::height::IHeight;

use super::identifier::{ClientId, Identifier};

pub(crate) const PATH_SEPARATOR: char = '/';

/// Path is used as a key for an object store in state
///
/// # Specs
///
/// <https://github.com/cosmos/ibc/tree/master/spec/core/ics-024-host-requirements#paths-identifiers-separators>
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    identifiers: Vec<Identifier>,
    path: String,
}

impl Path {
    /// Applies the given prefix to path
    pub fn apply_prefix(&mut self, prefix: Identifier) {
        self.identifiers.insert(0, prefix);
    }
}

impl FromStr for Path {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(!s.trim().is_empty(), "path cannot be empty");

        let identifiers = s
            .split(PATH_SEPARATOR)
            .map(FromStr::from_str)
            .collect::<Result<Vec<Identifier>, _>>()?;

        ensure!(
            identifiers.len() > 1,
            "path {} doesn't contain any separator '{}'",
            s,
            PATH_SEPARATOR
        );

        Ok(Self {
            identifiers,
            path: s.to_owned(),
        })
    }
}

fn compute_path(identifiers: &[Identifier]) -> Result<String, Error> {
    ensure!(
        identifiers.len() > 1,
        "path contains less than or equal to one identifier"
    );

    let mut path = identifiers[0].to_string();

    for id in identifiers.iter().skip(1) {
        path.push_str(&id.to_string());
    }

    Ok(path)
}

impl AsRef<[u8]> for Path {
    fn as_ref(&self) -> &[u8] {
        self.path.as_bytes()
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl From<Path> for MerklePath {
    fn from(value: Path) -> Self {
        let key_path = value.identifiers.into_iter().map(Into::into).collect();
        MerklePath { key_path }
    }
}

impl From<Path> for Vec<Identifier> {
    fn from(path: Path) -> Self {
        path.identifiers
    }
}

impl TryFrom<Vec<Identifier>> for Path {
    type Error = Error;

    fn try_from(identifiers: Vec<Identifier>) -> Result<Self, Self::Error> {
        let path = compute_path(&identifiers)?;
        Ok(Self { identifiers, path })
    }
}

impl TryFrom<&MerklePath> for Path {
    type Error = Error;

    fn try_from(value: &MerklePath) -> Result<Self, Self::Error> {
        let identifiers = value
            .key_path
            .iter()
            .map(|id| id.parse())
            .collect::<Result<Vec<Identifier>, _>>()?;

        identifiers.try_into()
    }
}

macro_rules! impl_path {
    ($doc: expr, $name: ident) => {
        #[doc = $doc]
        pub struct $name(Path);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Deref for $name {
            type Target = Path;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

impl_path!("Path for storing client type", ClientTypePath);

impl ClientTypePath {
    /// Creates a new client type path from client id
    pub fn new(client_id: ClientId) -> Self {
        Self(
            format!(
                "clients{}{}{}clientType",
                PATH_SEPARATOR, client_id, PATH_SEPARATOR
            )
            .parse()
            .unwrap(),
        )
    }
}

impl_path!("Path for storing client state", ClientStatePath);

impl ClientStatePath {
    /// Creates a new client state path from client id
    pub fn new(client_id: &ClientId) -> Self {
        Self(
            format!(
                "clients{}{}{}clientState",
                PATH_SEPARATOR, client_id, PATH_SEPARATOR
            )
            .parse()
            .unwrap(),
        )
    }
}

impl_path!("Path for storing consensus state", ConsensusStatePath);

impl ConsensusStatePath {
    /// Creates a new consensus state path from client id and height
    pub fn new(client_id: &ClientId, height: Height) -> Self {
        Self(
            format!(
                "clients{}{}{}consensusStates{}{}",
                PATH_SEPARATOR,
                client_id,
                PATH_SEPARATOR,
                PATH_SEPARATOR,
                height.to_string()
            )
            .parse()
            .unwrap(),
        )
    }
}
