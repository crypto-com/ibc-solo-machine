use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::{
    core::{
        commitment::v1::MerklePrefix,
        connection::v1::{
            ConnectionEnd, Counterparty as ConnectionCounterparty, State,
            Version as ConnectionVersion,
        },
    },
    lightclients::tendermint::v1::{ClientState, ConsensusState},
};
use ibc::{
    core::{
        ics02_client::{client_type::ClientType, height::IHeight},
        ics24_host::{
            identifier::{ClientId, ConnectionId},
            path::{ClientStatePath, ConnectionPath, ConsensusStatePath},
        },
    },
    proto::proto_encode,
};
use prost::Message;
use sled::Tree;

pub struct MsgHandler {
    tree: Tree,
}

impl MsgHandler {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn create_client(
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

    pub fn connection_open_try(
        &self,
        tendermint_client_id: &ClientId,
        solo_machine_client_id: &ClientId,
        solo_machine_connection_id: &ConnectionId,
    ) -> Result<ConnectionId> {
        let connection_id = ConnectionId::generate();

        let connection = ConnectionEnd {
            client_id: tendermint_client_id.to_string(),
            counterparty: Some(ConnectionCounterparty {
                client_id: solo_machine_client_id.to_string(),
                connection_id: solo_machine_connection_id.to_string(),
                prefix: Some(MerklePrefix {
                    key_prefix: "ibc".as_bytes().to_vec(),
                }),
            }),
            versions: vec![ConnectionVersion {
                identifier: "1".to_string(),
                features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
            }],
            state: State::Tryopen.into(),
            delay_period: 0,
        };

        let connection_path = ConnectionPath::new(&connection_id);

        self.tree
            .insert(&connection_path, proto_encode(&connection)?)?;

        Ok(connection_id)
    }

    pub fn connection_open_confirm(&self, connection_id: &ConnectionId) -> Result<()> {
        let connection_path = ConnectionPath::new(&connection_id);

        let connection_bytes = self.tree.get(&connection_path)?.ok_or_else(|| {
            anyhow!(
                "connection details for connection id {} not found",
                connection_id
            )
        })?;
        let mut connection = ConnectionEnd::decode(connection_bytes.as_ref())?;
        connection.set_state(State::Open);

        self.tree
            .insert(&connection_path, proto_encode(&connection)?)?;

        Ok(())
    }
}
