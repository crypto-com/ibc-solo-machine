use anyhow::Result;
use cosmos_sdk_proto::ibc::{
    core::{client::v1::Height, connection::v1::ConnectionEnd},
    lightclients::tendermint::v1::{
        ClientState as TendermintClientState, ConsensusState as TendermintConsensusState,
    },
};
use ibc::core::ics24_host::{
    identifier::{ClientId, ConnectionId},
    path::{ClientStatePath, ConnectionPath, ConsensusStatePath},
};
use prost::Message;
use sled::Tree;

pub struct QueryHandler {
    tree: Tree,
}

impl QueryHandler {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn get_client_state(&self, client_id: &ClientId) -> Result<Option<TendermintClientState>> {
        let path = ClientStatePath::new(client_id);
        let bytes = self.tree.get(&path)?;

        match bytes {
            None => Ok(None),
            Some(bytes) => {
                let client_state = TendermintClientState::decode(bytes.as_ref())?;
                Ok(Some(client_state))
            }
        }
    }

    pub fn get_consensus_state(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Option<TendermintConsensusState>> {
        let path = ConsensusStatePath::new(client_id, height);
        let bytes = self.tree.get(&path)?;

        match bytes {
            None => Ok(None),
            Some(bytes) => {
                let consensus_state = TendermintConsensusState::decode(bytes.as_ref())?;
                Ok(Some(consensus_state))
            }
        }
    }

    pub fn get_connection(&self, connection_id: &ConnectionId) -> Result<Option<ConnectionEnd>> {
        let path = ConnectionPath::new(connection_id);
        let bytes = self.tree.get(&path)?;

        match bytes {
            None => Ok(None),
            Some(bytes) => {
                let connection = ConnectionEnd::decode(bytes.as_ref())?;
                Ok(Some(connection))
            }
        }
    }
}
