use std::ops::{Deref, DerefMut};

use anyhow::{ensure, Error};
use ibc_proto::ibc::core::{client::v1::Height, commitment::v1::MerklePath};

use crate::ibc::core::ics02_client::height::IHeight;

use super::identifier::{ChannelId, ClientId, ConnectionId, Identifier, PortId};

/// Path is used as a key for an object store in state
///
/// # Specs
///
/// <https://github.com/cosmos/ibc/tree/master/spec/core/ics-024-host-requirements#paths-identifiers-separators>
#[derive(Debug, Clone)]
pub struct Path(MerklePath);

impl Path {
    /// Returns a new path from the given key path
    pub fn new_from_str(key_path: String) -> Self {
        Path(MerklePath {
            key_path: vec![key_path],
        })
    }
    /// Applies the given prefix to path
    pub fn apply_prefix(&mut self, prefix: &str) -> Result<(), Error> {
        ensure!(!prefix.trim().is_empty(), "prefix cannot be empty");
        self.0.key_path.insert(0, prefix.trim().to_string());
        Ok(())
    }

    /// Returns the key at given index
    pub fn get_key(&self, index: usize) -> Option<&str> {
        self.0.key_path.get(index).map(AsRef::as_ref)
    }

    /// Returns the length of the path
    pub fn len(&self) -> usize {
        self.0.key_path.len()
    }

    /// Checks if the path is empty
    pub fn is_empty(&self) -> bool {
        self.0.key_path.is_empty()
    }
}

impl Deref for Path {
    type Target = MerklePath;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Path {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// impl FromStr for Path {
//     type Err = Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         ensure!(!s.trim().is_empty(), "path cannot be empty");

//         Ok(Path(MerklePath {
//             key_path: vec![s.trim().to_string()],
//         }))
//     }
// }

// impl fmt::Display for Path {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         for key in &self.0.key_path {
//             write!(f, "/{}", urlencoding::encode(key))?;
//         }

//         Ok(())
//     }
// }

macro_rules! impl_path {
    ($doc: expr, $name: ident) => {
        #[doc = $doc]
        #[derive(Debug, Clone)]
        pub struct $name(Path);

        impl $name {
            /// Applies the given prefix to path
            #[allow(dead_code)]
            pub fn with_prefix(self, prefix: &str) -> Result<Self, Error> {
                let mut path = self.0;
                path.apply_prefix(prefix)?;

                Ok(Self(path))
            }
        }

        // impl fmt::Display for $name {
        //     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //         write!(f, "{}", self.0)
        //     }
        // }

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
        Self(Path::new_from_str(format!(
            "clients/{}/clientType",
            client_id
        )))
    }
}

impl_path!("Path for storing client state", ClientStatePath);

impl ClientStatePath {
    /// Creates a new client state path from client id
    pub fn new(client_id: &ClientId) -> Self {
        Self(Path::new_from_str(format!(
            "clients/{}/clientState",
            client_id
        )))
    }
}

impl_path!("Path for storing consensus state", ConsensusStatePath);

impl ConsensusStatePath {
    /// Creates a new consensus state path from client id and height
    pub fn new(client_id: &ClientId, height: &Height) -> Self {
        Self(Path::new_from_str(format!(
            "clients/{}/consensusStates/{}",
            client_id,
            height.to_string()
        )))
    }
}

impl_path!("Path for storing connection", ConnectionPath);

impl ConnectionPath {
    pub fn new(connection_id: &ConnectionId) -> Self {
        Self(Path::new_from_str(format!("connections/{}", connection_id)))
    }
}

impl_path!("Path for storing channel", ChannelPath);

impl ChannelPath {
    pub fn new(port_id: &PortId, channel_id: &ChannelId) -> Self {
        Self(Path::new_from_str(format!(
            "channelEnds/ports/{}/channels/{}",
            port_id, channel_id
        )))
    }
}

impl_path!("Path for storing packet commitments", PacketCommitmentPath);

impl PacketCommitmentPath {
    pub fn new(port_id: &PortId, channel_id: &ChannelId, packet_sequence: u64) -> Self {
        Self(Path::new_from_str(format!(
            "commitments/ports/{}/channels/{}/sequences/{}",
            port_id, channel_id, packet_sequence
        )))
    }
}

impl_path!("Denom trace of tokens transferred to IBC chain", DenomTrace);

impl DenomTrace {
    pub fn new(port_id: &PortId, channel_id: &ChannelId, denom: &Identifier) -> Self {
        Self(Path::new_from_str(format!(
            "{}/{}/{}",
            port_id, channel_id, denom
        )))
    }
}

impl_path!(
    "Path for storing packet acknowledgements",
    PacketAcknowledgementPath
);

impl PacketAcknowledgementPath {
    pub fn new(port_id: &PortId, channel_id: &ChannelId, packet_sequence: u64) -> Self {
        Self(Path::new_from_str(format!(
            "acks/ports/{}/channels/{}/sequences/{}",
            port_id, channel_id, packet_sequence
        )))
    }
}
