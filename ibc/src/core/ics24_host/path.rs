use std::{
    convert::{TryFrom, TryInto},
    fmt,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use anyhow::{ensure, Error};
use cosmos_sdk_proto::ibc::core::{client::v1::Height, commitment::v1::MerklePath};

use crate::core::ics02_client::height::IHeight;

use super::identifier::{ChannelId, ClientId, ConnectionId, Identifier, PortId};

/// Path is used as a key for an object store in state
///
/// # Specs
///
/// <https://github.com/cosmos/ibc/tree/master/spec/core/ics-024-host-requirements#paths-identifiers-separators>
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(String);

impl Path {
    /// Applies the given prefix to path
    pub fn apply_prefix(&mut self, prefix: &Identifier) {
        let path = format!(
            "/{}/{}",
            urlencoding::encode(&prefix),
            urlencoding::encode(&self.0)
        );

        self.0 = path;
    }

    /// Returns bytes of current path
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
}

impl FromStr for Path {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(!s.trim().is_empty(), "path cannot be empty");

        let identifiers = s
            .split('/')
            .map(FromStr::from_str)
            .collect::<Result<Vec<Identifier>, _>>()?;

        ensure!(
            identifiers.len() > 1,
            "path {} doesn't contain any separator '/'",
            s,
        );

        Ok(Self(s.to_owned()))
    }
}

fn compute_path(identifiers: &[Identifier]) -> Result<String, Error> {
    ensure!(
        identifiers.len() > 1,
        "path contains less than or equal to one identifier"
    );

    let mut path = String::new();

    for id in identifiers.iter() {
        path.push_str(&format!("/{}", id));
    }

    Ok(path)
}

impl AsRef<[u8]> for Path {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Path> for MerklePath {
    fn from(value: Path) -> Self {
        MerklePath {
            key_path: vec![value.0],
        }
    }
}

impl TryFrom<&[Identifier]> for Path {
    type Error = Error;

    fn try_from(identifiers: &[Identifier]) -> Result<Self, Self::Error> {
        let path = compute_path(identifiers)?;
        Ok(Self(path))
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

        identifiers.as_slice().try_into()
    }
}

macro_rules! impl_path {
    ($doc: expr, $name: ident) => {
        #[doc = $doc]
        pub struct $name(Path);

        impl $name {
            pub fn into_bytes(self) -> Vec<u8> {
                self.0.into_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }

        impl Deref for $name {
            type Target = Path;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

impl_path!("Path for storing client type", ClientTypePath);

impl ClientTypePath {
    /// Creates a new client type path from client id
    pub fn new(client_id: ClientId) -> Self {
        Self(format!("clients/{}/clientType", client_id).parse().unwrap())
    }
}

impl_path!("Path for storing client state", ClientStatePath);

impl ClientStatePath {
    /// Creates a new client state path from client id
    pub fn new(client_id: &ClientId) -> Self {
        Self(
            format!("clients/{}/clientState", client_id)
                .parse()
                .unwrap(),
        )
    }
}

impl_path!("Path for storing consensus state", ConsensusStatePath);

impl ConsensusStatePath {
    /// Creates a new consensus state path from client id and height
    pub fn new(client_id: &ClientId, height: &Height) -> Self {
        Self(
            format!(
                "clients/{}/consensusStates/{}",
                client_id,
                height.to_string()
            )
            .parse()
            .unwrap(),
        )
    }
}

impl_path!("Path for storing connection", ConnectionPath);

impl ConnectionPath {
    pub fn new(connection_id: &ConnectionId) -> Self {
        Self(format!("connections/{}", connection_id).parse().unwrap())
    }
}

impl_path!("Path for storing channel", ChannelPath);

impl ChannelPath {
    pub fn new(port_id: &PortId, channel_id: &ChannelId) -> Self {
        Self(
            format!("channelEnds/ports/{}/channels/{}", port_id, channel_id)
                .parse()
                .unwrap(),
        )
    }
}

impl_path!("Path for storing packet commitments", PacketCommitmentPath);

impl PacketCommitmentPath {
    pub fn new(port_id: &PortId, channel_id: &ChannelId, packet_sequence: u64) -> Self {
        Self(
            format!(
                "commitments/ports/{}/channels/{}/sequences/{}",
                port_id, channel_id, packet_sequence
            )
            .parse()
            .unwrap(),
        )
    }
}

impl_path!("Denom trace of tokens transferred to IBC chain", DenomTrace);

impl DenomTrace {
    pub fn new(port_id: &PortId, channel_id: &ChannelId, denom: &Identifier) -> Self {
        Self(
            format!("{}/{}/{}", port_id, channel_id, denom)
                .parse()
                .unwrap(),
        )
    }
}
