tonic::include_proto!("ibc");

use std::collections::HashMap;

use anyhow::{anyhow, ensure, Context, Error};
use bip39::{Language, Mnemonic};
use cosmos_sdk_proto::ibc::{
    applications::transfer::v1::FungibleTokenPacketData,
    core::{channel::v1::Packet, client::v1::Height},
};
use ibc::{
    core::{
        ics02_client::height::IHeight,
        ics24_host::identifier::{ChainId, ChannelId, ClientId, ConnectionId},
    },
    proto::proto_encode,
};
use tendermint::{
    abci::{
        responses::Event,
        tag::{Key, Tag},
    },
    trust_threshold::TrustThresholdFraction,
    Hash as TendermintHash,
};
use tendermint_light_client::{
    builder::LightClientBuilder, light_client::Options, store::memory::MemoryStore,
    supervisor::Instance,
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

use super::{
    bank::BankService,
    chain::{Chain, ChainConnectionDetails, ChainService},
};

const DEFAULT_MEMO: &str = "solo-machine-memo";

pub struct IbcService {
    msg_handler: MsgHandler,
    query_handler: QueryHandler,
    chain_service: ChainService,
    bank_service: BankService,
}

impl IbcService {
    /// Creates a new instance of ibc service
    pub fn new(
        msg_handler: MsgHandler,
        query_handler: QueryHandler,
        chain_service: ChainService,
        bank_service: BankService,
    ) -> Self {
        Self {
            msg_handler,
            query_handler,
            chain_service,
            bank_service,
        }
    }

    async fn send_to_chain(
        &self,
        chain_id: ChainId,
        mnemonic: Mnemonic,
        memo: String,
        amount: u64,
        denom: String,
        receiver: String,
    ) -> Result<(), Error> {
        let chain = self
            .chain_service
            .get(&chain_id)?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        self.bank_service
            .burn(&mnemonic, &chain.account_prefix, amount.into(), &denom)?;

        let rpc_client =
            HttpClient::new(chain.rpc_addr.as_str()).context("unable to connect to rpc client")?;
        let transaction_builder =
            TransactionBuilder::new(&self.chain_service, &chain_id, &mnemonic, &memo);

        let msg = transaction_builder
            .msg_token_send(&rpc_client, amount, denom, receiver)
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        Ok(())
    }

    async fn receive_from_chain(
        &self,
        chain_id: ChainId,
        mnemonic: Mnemonic,
        memo: String,
        amount: u64,
        denom: String,
        receiver: String,
    ) -> Result<(), Error> {
        let chain = self
            .chain_service
            .get(&chain_id)?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        let rpc_client =
            HttpClient::new(chain.rpc_addr.as_str()).context("unable to connect to rpc client")?;
        let transaction_builder =
            TransactionBuilder::new(&self.chain_service, &chain_id, &mnemonic, &memo);

        let msg = transaction_builder.msg_update_solo_machine_client().await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        let msg = transaction_builder
            .msg_token_receive(amount, &denom, receiver)
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        self.process_packets(
            &rpc_client,
            &transaction_builder,
            &chain,
            extract_packets(&response)?,
        )
        .await?;

        Ok(())
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

        let rpc_client =
            HttpClient::new(chain.rpc_addr.as_str()).context("unable to connect to rpc client")?;
        let transaction_builder =
            TransactionBuilder::new(&self.chain_service, &chain_id, &mnemonic, &memo);
        let mut instance = prepare_light_client(&chain, rpc_client.clone())?;

        let solo_machine_client_id = self
            .create_solo_machine_client(&rpc_client, &transaction_builder)
            .await?;

        log::info!("Created solo machine client: {}", solo_machine_client_id);

        let tendermint_client_id = self
            .create_tendermint_client(&mut instance, &transaction_builder)
            .await?;

        log::info!("Created tendermint client: {}", tendermint_client_id);

        let solo_machine_connection_id = self
            .connection_open_init(
                &rpc_client,
                &transaction_builder,
                &solo_machine_client_id,
                &tendermint_client_id,
            )
            .await?;

        log::info!(
            "Initialized solo machine connection: {}",
            solo_machine_connection_id
        );

        let tendermint_connection_id = self.msg_handler.connection_open_try(
            &tendermint_client_id,
            &solo_machine_client_id,
            &solo_machine_connection_id,
        )?;

        log::info!(
            "Initialized tendermint connection: {}",
            tendermint_connection_id
        );

        self.connection_open_ack(
            &rpc_client,
            &transaction_builder,
            &solo_machine_connection_id,
            &tendermint_client_id,
            &tendermint_connection_id,
        )
        .await?;

        log::info!("Sent connection open acknowledgement");

        self.msg_handler
            .connection_open_confirm(&tendermint_connection_id)?;

        log::info!("Sent connection open confirmation");

        let solo_machine_channel_id = self
            .channel_open_init(
                &rpc_client,
                &transaction_builder,
                &solo_machine_connection_id,
            )
            .await?;

        log::info!(
            "Initialized solo machine channel: {}",
            solo_machine_channel_id
        );

        let tendermint_channel_id = self.msg_handler.channel_open_try(
            &chain.port_id,
            &solo_machine_channel_id,
            &tendermint_connection_id,
        )?;

        log::info!("Initialized tendermint channel: {}", tendermint_channel_id);

        self.channel_open_ack(
            &rpc_client,
            &transaction_builder,
            &solo_machine_channel_id,
            &tendermint_channel_id,
        )
        .await?;

        log::info!("Sent channel open acknowledgement");

        self.msg_handler
            .channel_open_confirm(&chain.port_id, &tendermint_channel_id)?;

        log::info!("Sent channel open confirmation");

        let connection_details = ChainConnectionDetails {
            solo_machine_client_id,
            tendermint_client_id,
            solo_machine_connection_id,
            tendermint_connection_id,
            solo_machine_channel_id,
            tendermint_channel_id,
        };

        self.chain_service
            .add_connection_details(&chain_id, &connection_details)?;

        Ok(())
    }

    async fn create_solo_machine_client<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder<'_>,
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

    async fn create_tendermint_client(
        &self,
        instance: &mut Instance,
        transaction_builder: &TransactionBuilder<'_>,
    ) -> Result<ClientId, Error> {
        let (client_state, consensus_state) = transaction_builder
            .msg_create_tendermint_client(instance)
            .await?;

        self.msg_handler
            .create_client(&client_state, &consensus_state)
    }

    async fn connection_open_init<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder<'_>,
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
        transaction_builder: &TransactionBuilder<'_>,
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

    async fn channel_open_init<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder<'_>,
        solo_machine_connection_id: &ConnectionId,
    ) -> Result<ChannelId, Error>
    where
        C: Client + Send + Sync,
    {
        let msg = transaction_builder
            .msg_channel_open_init(solo_machine_connection_id)
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        extract_attribute(
            &response.deliver_tx.events,
            "channel_open_init",
            "channel_id",
        )?
        .parse()
    }

    async fn channel_open_ack<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder<'_>,
        solo_machine_channel_id: &ChannelId,
        tendermint_channel_id: &ChannelId,
    ) -> Result<(), Error>
    where
        C: Client + Send + Sync,
    {
        let msg = transaction_builder
            .msg_channel_open_ack(
                &self.query_handler,
                solo_machine_channel_id,
                tendermint_channel_id,
            )
            .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        Ok(())
    }

    async fn process_packets<C>(
        &self,
        rpc_client: &C,
        transaction_builder: &TransactionBuilder<'_>,
        chain: &Chain,
        packets: Vec<Packet>,
    ) -> Result<(), Error>
    where
        C: Client + Send + Sync,
    {
        let connection_details = chain.connection_details.as_ref().ok_or_else(|| {
            anyhow!(
                "connection details for chain with id {} are missing",
                chain.id
            )
        })?;

        for packet in packets {
            ensure!(
                chain.port_id.to_string() == packet.source_port,
                "invalid source port id"
            );
            ensure!(
                connection_details.solo_machine_channel_id.to_string() == packet.source_channel,
                "invalid source channel id"
            );
            ensure!(
                chain.port_id.to_string() == packet.destination_port,
                "invalid destination port id"
            );
            ensure!(
                connection_details.tendermint_channel_id.to_string() == packet.destination_channel,
                "invalid destination channel id"
            );

            let packet_data = parse_packet_data(&packet.data)?;

            let msg = transaction_builder.msg_token_receive_ack(packet).await?;

            let response = rpc_client
                .broadcast_tx_commit(proto_encode(&msg)?.into())
                .await?;

            ensure_response_success(&response)?;

            self.bank_service.mint_to(
                &packet_data.receiver,
                packet_data.amount.into(),
                packet_data
                    .denom
                    .split('/')
                    .last()
                    .ok_or_else(|| anyhow!("unable to parse denom in packet data"))?,
            )?;
        }

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
            .map_err(|e| Status::internal(format!("{:?}", e)))?;

        Ok(Response::new(Default::default()))
    }

    async fn send_to_chain(
        &self,
        request: Request<SendToChainRequest>,
    ) -> Result<Response<SendToChainResponse>, Status> {
        let request = request.into_inner();

        let chain_id: ChainId = request
            .chain_id
            .parse()
            .map_err(|e: Error| Status::invalid_argument(e.to_string()))?;

        let mnemonic: Mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_string());

        let amount = request.amount;
        let denom = request.denom;

        let receiver_address = request.receiver_address;

        self.send_to_chain(chain_id, mnemonic, memo, amount, denom, receiver_address)
            .await
            .map_err(|e| Status::internal(format!("{:?}", e)))?;

        Ok(Response::new(Default::default()))
    }

    async fn receive_from_chain(
        &self,
        request: Request<ReceiveFromChainRequest>,
    ) -> Result<Response<ReceiveFromChainResponse>, Status> {
        let request = request.into_inner();

        let chain_id: ChainId = request
            .chain_id
            .parse()
            .map_err(|e: Error| Status::invalid_argument(e.to_string()))?;

        let mnemonic: Mnemonic = Mnemonic::from_phrase(&request.mnemonic, Language::English)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_string());

        let amount = request.amount;
        let denom = request.denom;

        let receiver_address = request.receiver_address;

        self.receive_from_chain(chain_id, mnemonic, memo, amount, denom, receiver_address)
            .await
            .map_err(|e| Status::internal(format!("{:?}", e)))?;

        Ok(Response::new(Default::default()))
    }
}

fn parse_packet_data(packet_data: &[u8]) -> Result<FungibleTokenPacketData, Error> {
    let mut packet_data: HashMap<String, String> =
        serde_json::from_slice(packet_data).context("invalid packet data")?;

    let data = FungibleTokenPacketData {
        denom: packet_data
            .remove("denom")
            .ok_or_else(|| anyhow!("`denom` is missing in packet data"))?,
        amount: packet_data
            .remove("amount")
            .ok_or_else(|| anyhow!("`amount` is missing in packet data"))?
            .parse()
            .context("invalid amount in packet data")?,
        receiver: packet_data
            .remove("receiver")
            .ok_or_else(|| anyhow!("`receiver` is missing in packet data"))?,
        sender: packet_data
            .remove("sender")
            .ok_or_else(|| anyhow!("`sender` is missing in packet data"))?,
    };

    Ok(data)
}

fn extract_packets(response: &TxCommitResponse) -> Result<Vec<Packet>, Error> {
    let mut packets = vec![];

    for event in response.deliver_tx.events.iter() {
        if event.type_str == "send_packet" {
            let mut attributes = HashMap::new();

            for tag in event.attributes.iter() {
                attributes.insert(tag.key.to_string(), tag.value.to_string());
            }

            let packet = Packet {
                sequence: attributes
                    .remove("packet_sequence")
                    .ok_or_else(|| anyhow!("`packet_sequence` is missing from packet data"))?
                    .parse()
                    .context("invalid `packet_sequence`")?,
                source_port: attributes
                    .remove("packet_src_port")
                    .ok_or_else(|| anyhow!("`packet_src_port` is missing from packet data"))?,
                source_channel: attributes
                    .remove("packet_src_channel")
                    .ok_or_else(|| anyhow!("`packet_src_channel` is missing from packet data"))?,
                destination_port: attributes
                    .remove("packet_dst_port")
                    .ok_or_else(|| anyhow!("`packet_dst_port` is missing from packet data"))?,
                destination_channel: attributes
                    .remove("packet_dst_channel")
                    .ok_or_else(|| anyhow!("`packet_dst_channel` is missing from packet data"))?,
                data: attributes
                    .remove("packet_data")
                    .ok_or_else(|| anyhow!("`packet_data` is missing from packet data"))?
                    .into_bytes(),
                timeout_height: Some(
                    Height::from_str(&attributes.remove("packet_timeout_height").ok_or_else(
                        || anyhow!("`packet_timeout_height` is missing from packet data"),
                    )?)
                    .context("invalid `packet_timeout_height`")?,
                ),
                timeout_timestamp: attributes
                    .remove("packet_timeout_timestamp")
                    .ok_or_else(|| {
                        anyhow!("`packet_timeout_timestamp` is missing from packet data")
                    })?
                    .parse()
                    .context("invalid `packet_timeout_timestamp`")?,
            };

            packets.push(packet);
        }
    }

    Ok(packets)
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

fn prepare_light_client(chain: &Chain, rpc_client: HttpClient) -> Result<Instance, Error> {
    let builder = LightClientBuilder::prod(
        chain.node_id,
        rpc_client,
        Box::new(MemoryStore::new()),
        Options {
            trust_threshold: TrustThresholdFraction::new(
                *chain.trust_level.numer(),
                *chain.trust_level.denom(),
            )
            .unwrap(),
            trusting_period: chain.trusting_period,
            clock_drift: chain.max_clock_drift,
        },
        Some(chain.rpc_timeout),
    );

    let builder =
        builder.trust_primary_at(chain.trusted_height, get_trusted_hash(chain.trusted_hash))?;

    Ok(builder.build())
}

fn get_trusted_hash(trusted_hash: Option<[u8; 32]>) -> TendermintHash {
    match trusted_hash {
        None => TendermintHash::None,
        Some(bytes) => TendermintHash::Sha256(bytes),
    }
}
