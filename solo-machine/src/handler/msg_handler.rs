use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::lightclients::tendermint::v1::{ClientState, ConsensusState};
use ibc::{
    core::{
        ics02_client::{client_type::ClientType, height::IHeight},
        ics24_host::{
            identifier::ClientId,
            path::{ClientStatePath, ConsensusStatePath},
        },
    },
    proto::proto_encode,
};
use sled::Tree;

pub trait MsgHandler {
    fn create_client(
        &self,
        client_state: &ClientState,
        consensus_state: &ConsensusState,
    ) -> Result<ClientId>;
}

pub struct SoloMachineMsgHandler {
    tree: Tree,
}

impl SoloMachineMsgHandler {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }
}

impl MsgHandler for SoloMachineMsgHandler {
    fn create_client(
        &self,
        client_state: &ClientState,
        consensus_state: &ConsensusState,
    ) -> Result<ClientId> {
        let client_id = ClientId::generate(ClientType::Tendermint);
        let latest_height = client_state
            .latest_height
            .as_ref()
            .ok_or_else(|| anyhow!("latest height cannot be absent in client state"))?;

        let client_state_path = ClientStatePath::new(&client_id);

        self.tree
            .insert(&client_state_path, proto_encode(client_state)?)?;

        log::info!(
            "client created with id: {}, and at height: {}",
            client_id,
            latest_height.to_string(),
        );

        let consensus_state_path = ConsensusStatePath::new(&client_id, latest_height);

        self.tree
            .insert(&consensus_state_path, proto_encode(consensus_state)?)?;

        Ok(client_id)
    }
}
