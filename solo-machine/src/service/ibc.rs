tonic::include_proto!("ibc");

use anyhow::{anyhow, ensure, Error};
use bip39::{Language, Mnemonic};
use ibc::{
    core::ics24_host::identifier::{ChainId, ClientId, ConnectionId},
    proto::proto_encode,
};
use tendermint::abci::{
    responses::Event,
    tag::{Key, Tag},
};
use tendermint_light_client::{
    components::{
        clock::SystemClock, io::ProdIo, scheduler::basic_bisecting_schedule, verifier::ProdVerifier,
    },
    light_client::{LightClient, Options},
    operations::hasher::ProdHasher,
    types::TrustThreshold,
};
use tendermint_rpc::{
    endpoint::broadcast::tx_commit::Response as TxCommitResponse, Client, HttpClient,
};
use tonic::{Request, Response, Status};

use crate::{
    handler::{msg_handler::MsgHandler, query_handler::QueryHandler},
    transaction_builder::TransactionBuilder,
};

use self::ibc_server::Ibc;

use super::chain::{Chain, ChainService};

const DEFAULT_MEMO: &str = "solo-machine-memo";

pub struct IbcService {
    msg_handler: MsgHandler,
    query_handler: QueryHandler,
    chain_service: ChainService,
}

impl IbcService {
    /// Creates a new instance of ibc service
    pub fn new(
        msg_handler: MsgHandler,
        query_handler: QueryHandler,
        chain_service: ChainService,
    ) -> Self {
        Self {
            msg_handler,
            query_handler,
            chain_service,
        }
    }

    async fn connect(
        &self,
        chain_id: ChainId,
        mnemonic: Mnemonic,
        memo: String,
    ) -> Result<(), Error> {
        let chain = self
            .chain_service
            .get(&chain_id)?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        let rpc_client = HttpClient::new(chain.rpc_addr.as_str())?;
        let light_client = prepare_light_client(&chain, rpc_client.clone());
        let light_client_io = prepare_light_client_io(&chain, rpc_client.clone());
        let transaction_builder = TransactionBuilder::new(chain, mnemonic, memo);

        let solo_machine_client_id = self
            .create_solo_machine_client(&rpc_client, &transaction_builder)
            .await?;

        let tendermint_client_id = self
            .create_tendermint_client(
                &rpc_client,
                &light_client,
                &light_client_io,
                &transaction_builder,
            )
            .await?;

        let solo_machine_connection_id = self
            .connection_open_init(
                &rpc_client,
                &transaction_builder,
                &solo_machine_client_id,
                &tendermint_client_id,
            )
            .await?;

        let tendermint_connection_id = self.msg_handler.connection_open_try(
            &tendermint_client_id,
            &solo_machine_client_id,
            &solo_machine_connection_id,
        )?;

        self.connection_open_ack(
            &rpc_client,
            &transaction_builder,
            &solo_machine_connection_id,
            &tendermint_client_id,
            &tendermint_connection_id,
        )
        .await?;

        self.msg_handler
            .connection_open_confirm(&tendermint_connection_id)?;

        Ok(())
    }

    async fn create_solo_machine_client<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder,
    ) -> Result<ClientId, Error>
    where
        C: Client + Send + Sync,
    {
        let msg = transaction_builder.msg_create_solo_machine_client().await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        extract_attribute(&response.deliver_tx.events, "create_client", "client_id")?.parse()
    }

    async fn create_tendermint_client<C>(
        &self,
        rpc_client: &C,
        light_client: &LightClient,
        light_client_io: &ProdIo,
        transaction_builder: &TransactionBuilder,
    ) -> Result<ClientId, Error>
    where
        C: Client + Send + Sync,
    {
        let (client_state, consensus_state) = transaction_builder
            .msg_create_tendermint_client(rpc_client, light_client, light_client_io)
            .await?;

        self.msg_handler
            .create_client(&client_state, &consensus_state)
    }

    async fn connection_open_init<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder,
        solo_machine_client_id: &ClientId,
        tendermint_client_id: &ClientId,
    ) -> Result<ConnectionId, Error>
    where
        C: Client + Send + Sync,
    {
        let msg = transaction_builder
            .msg_connection_open_init(solo_machine_client_id, tendermint_client_id)
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        extract_attribute(
            &response.deliver_tx.events,
            "connection_open_init",
            "connection_id",
        )?
        .parse()
    }

    async fn connection_open_ack<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder,
        solo_machine_connection_id: &ConnectionId,
        tendermint_client_id: &ClientId,
        tendermint_connection_id: &ConnectionId,
    ) -> Result<(), Error>
    where
        C: Client + Send + Sync,
    {
        let msg = transaction_builder
            .msg_connection_open_ack(
                &self.query_handler,
                solo_machine_connection_id,
                tendermint_client_id,
                tendermint_connection_id,
            )
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        Ok(())
    }
}

#[tonic::async_trait]
impl Ibc for IbcService {
    async fn connect(
        &self,
        request: Request<ConnectRequest>,
    ) -> Result<Response<ConnectResponse>, Status> {
        let request = request.into_inner();

        let chain_id: ChainId = request
            .chain_id
            .parse()
            .map_err(|e: Error| Status::invalid_argument(e.to_string()))?;

        let mnemonic: Mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_string());

        self.connect(chain_id, mnemonic, memo)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Default::default()))
    }
}

fn ensure_response_success(response: &TxCommitResponse) -> Result<(), Error> {
    ensure!(
        response.check_tx.code.is_ok(),
        "check_tx response contains error code: {:?}",
        response
    );

    ensure!(
        response.deliver_tx.code.is_ok(),
        "deliver_tx response contains error code: {:?}",
        response
    );

    Ok(())
}

fn extract_attribute(events: &[Event], event_type: &str, key: &str) -> Result<String, Error> {
    let mut attribute = None;

    for event in events {
        if event.type_str == event_type {
            attribute = Some(get_attribute(&event.attributes, key)?);
        }
    }

    attribute.ok_or_else(|| {
        anyhow!(
            "{}:{} not found in tendermint response events: {:?}",
            event_type,
            key,
            events
        )
    })
}

fn get_attribute(tags: &[Tag], key: &str) -> Result<String, Error> {
    let key: Key = key
        .parse()
        .map_err(|e| anyhow!("unable to parse attribute key `{}`: {}", key, e))?;

    for tag in tags {
        if tag.key == key {
            return Ok(tag.value.to_string());
        }
    }

    Err(anyhow!("{} not found in tags: {:?}", key, tags))
}

fn prepare_light_client(chain: &Chain, rpc_client: HttpClient) -> LightClient {
    LightClient::new(
        chain.node_id,
        Options {
            trust_threshold: TrustThreshold::new(
                *chain.trust_level.numer(),
                *chain.trust_level.denom(),
            )
            .unwrap(),
            trusting_period: chain.trusting_period,
            clock_drift: chain.max_clock_drift,
        },
        SystemClock,
        basic_bisecting_schedule,
        ProdVerifier::default(),
        ProdHasher,
        prepare_light_client_io(chain, rpc_client),
    )
}

fn prepare_light_client_io(chain: &Chain, rpc_client: HttpClient) -> ProdIo {
    ProdIo::new(chain.node_id, rpc_client, Some(chain.rpc_timeout))
}
