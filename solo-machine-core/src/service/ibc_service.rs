use std::collections::HashMap;

use anyhow::{anyhow, ensure, Context, Result};
use cosmos_sdk_proto::ibc::core::{
    channel::v1::{
        Channel, Counterparty as ChannelCounterparty, Order as ChannelOrder, Packet,
        State as ChannelState,
    },
    client::v1::Height,
    commitment::v1::MerklePrefix,
    connection::v1::{
        ConnectionEnd, Counterparty as ConnectionCounterparty, State as ConnectionState,
        Version as ConnectionVersion,
    },
};
use sqlx::{Executor, Transaction};
use tendermint::{
    abci::{
        tag::{Key, Tag},
        Event as AbciEvent,
    },
    trust_threshold::TrustThresholdFraction,
    Hash as TendermintHash,
};
use tendermint_light_client::{
    builder::LightClientBuilder, light_client::Options, store::memory::MemoryStore,
    store::LightStore, supervisor::Instance,
};
use tendermint_rpc::{
    endpoint::broadcast::tx_commit::Response as TxCommitResponse, Client, HttpClient,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    cosmos::crypto::PublicKey,
    event::{notify_event, Event},
    ibc::core::{
        ics02_client::{client_type::ClientType, height::IHeight},
        ics24_host::identifier::{ChainId, ChannelId, ClientId, ConnectionId, Identifier, PortId},
    },
    model::{
        chain::{self, chain_keys},
        ibc as ibc_handler,
        operation::{self, Operation},
        Chain, ConnectionDetails as ChainConnectionDetails, OperationType,
    },
    proto::proto_encode,
    transaction_builder, Db, DbPool, Signer, ToPublicKey,
};

/// Used to connect, send tokens and receive tokens over IBC
pub struct IbcService {
    db_pool: DbPool,
    notifier: Option<UnboundedSender<Event>>,
}

impl IbcService {
    /// Creates a new instance of IBC service
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            notifier: None,
        }
    }

    /// Creates a new instance of IBC service with notifier
    pub fn new_with_notifier(db_pool: DbPool, notifier: UnboundedSender<Event>) -> Self {
        Self {
            db_pool,
            notifier: Some(notifier),
        }
    }

    /// Establishes connection with an IBC enabled chain
    pub async fn connect(
        &self,
        signer: impl Signer,
        chain_id: ChainId,
        memo: String,
        force: bool,
    ) -> Result<()> {
        let mut transaction = self
            .db_pool
            .begin()
            .await
            .context("unable to begin database transaction")?;

        let mut chain = chain::get_chain(&mut transaction, &chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        if !force {
            ensure!(
                chain.connection_details.is_none(),
                "connection is already established with given chain"
            );
        }

        let rpc_client = HttpClient::new(chain.config.rpc_addr.as_str())
            .context("unable to connect to rpc client")?;
        let mut instance =
            prepare_light_client(&chain, rpc_client.clone(), Box::new(MemoryStore::new()))?;

        let solo_machine_client_id =
            create_solo_machine_client(&signer, &rpc_client, &chain, memo.clone()).await?;

        notify_event(
            &self.notifier,
            Event::CreatedSoloMachineClient {
                client_id: solo_machine_client_id.clone(),
            },
        )?;

        let tendermint_client_id =
            create_tendermint_client(&mut transaction, &mut instance, &chain).await?;

        notify_event(
            &self.notifier,
            Event::CreatedTendermintClient {
                client_id: tendermint_client_id.clone(),
            },
        )?;

        let solo_machine_connection_id = connection_open_init(
            &signer,
            &rpc_client,
            &chain,
            &solo_machine_client_id,
            &tendermint_client_id,
            memo.clone(),
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::InitializedConnectionOnTendermint {
                connection_id: solo_machine_connection_id.clone(),
            },
        )?;

        let tendermint_connection_id = connection_open_try(
            &mut transaction,
            &tendermint_client_id,
            &solo_machine_client_id,
            &solo_machine_connection_id,
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::InitializedConnectionOnSoloMachine {
                connection_id: tendermint_connection_id.clone(),
            },
        )?;

        connection_open_ack(
            &mut transaction,
            &signer,
            &rpc_client,
            &mut chain,
            &solo_machine_connection_id,
            &tendermint_client_id,
            &tendermint_connection_id,
            memo.clone(),
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::ConfirmedConnectionOnTendermint {
                connection_id: solo_machine_connection_id.clone(),
            },
        )?;

        connection_open_confirm(&mut transaction, &tendermint_connection_id).await?;

        notify_event(
            &self.notifier,
            Event::ConfirmedConnectionOnSoloMachine {
                connection_id: tendermint_connection_id.clone(),
            },
        )?;

        let solo_machine_channel_id = channel_open_init(
            &signer,
            &rpc_client,
            &chain,
            &solo_machine_connection_id,
            memo.clone(),
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::InitializedChannelOnTendermint {
                channel_id: solo_machine_channel_id.clone(),
            },
        )?;

        let tendermint_channel_id = channel_open_try(
            &mut transaction,
            &chain.config.port_id,
            &solo_machine_channel_id,
            &tendermint_connection_id,
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::InitializedChannelOnSoloMachine {
                channel_id: tendermint_channel_id.clone(),
            },
        )?;

        channel_open_ack(
            &mut transaction,
            signer,
            &rpc_client,
            &mut chain,
            &solo_machine_channel_id,
            &tendermint_channel_id,
            memo,
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::ConfirmedChannelOnTendermint {
                channel_id: solo_machine_channel_id.clone(),
            },
        )?;

        channel_open_confirm(
            &mut transaction,
            &chain.config.port_id,
            &tendermint_channel_id,
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::ConfirmedChannelOnSoloMachine {
                channel_id: tendermint_channel_id.clone(),
            },
        )?;

        let connection_details = ChainConnectionDetails {
            solo_machine_client_id,
            tendermint_client_id,
            solo_machine_connection_id,
            tendermint_connection_id,
            solo_machine_channel_id,
            tendermint_channel_id,
        };

        chain::add_connection_details(&mut transaction, &chain.id, &connection_details).await?;

        notify_event(
            &self.notifier,
            Event::ConnectionEstablished {
                chain_id,
                connection_details,
            },
        )?;

        transaction
            .commit()
            .await
            .context("unable to commit transaction for creating ibc connection")
    }

    /// Mint some tokens on IBC enabled chain
    #[allow(clippy::too_many_arguments)]
    pub async fn mint(
        &self,
        signer: impl Signer,
        chain_id: ChainId,
        request_id: Option<String>,
        amount: u32,
        denom: Identifier,
        receiver: Option<String>,
        memo: String,
    ) -> Result<String> {
        let mut chain = chain::get_chain(&self.db_pool, &chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        let address = signer.to_account_address()?;
        let receiver = receiver.unwrap_or_else(|| address.clone());

        let rpc_client = HttpClient::new(chain.config.rpc_addr.as_str())
            .context("unable to connect to rpc client")?;

        let mut transaction = self
            .db_pool
            .begin()
            .await
            .context("unable to begin database transaction")?;

        let msg = transaction_builder::msg_token_send(
            &mut transaction,
            signer,
            &rpc_client,
            &mut chain,
            amount,
            &denom,
            receiver.clone(),
            memo,
            request_id.as_deref(),
        )
        .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        let transaction_hash = ensure_response_success(&response)?;

        transaction
            .commit()
            .await
            .context("unable to commit transaction for sending tokens over IBC")?;

        let success: bool = extract_attribute(
            &response.deliver_tx.events,
            "fungible_token_packet",
            "success",
        )?
        .parse()?;

        if success {
            operation::add_operation(
                &self.db_pool,
                request_id.as_deref(),
                &receiver,
                &denom,
                amount,
                &OperationType::Mint {
                    chain_id: chain_id.clone(),
                },
                &transaction_hash,
            )
            .await?;

            notify_event(
                &self.notifier,
                Event::TokensMinted {
                    chain_id,
                    request_id,
                    to_address: receiver,
                    amount,
                    denom,
                    transaction_hash: transaction_hash.clone(),
                },
            )?;

            Ok(transaction_hash)
        } else {
            let error = extract_attribute(
                &response.deliver_tx.events,
                "write_acknowledgement",
                "packet_ack",
            )?;

            Err(anyhow!(
                "Failed to mint tokens on IBC enabled chain: {}",
                error
            ))
        }
    }

    /// Burn some tokens on IBC enabled chain
    #[allow(clippy::too_many_arguments)]
    pub async fn burn(
        &self,
        signer: impl Signer,
        chain_id: ChainId,
        request_id: Option<String>,
        amount: u32,
        denom: Identifier,
        receiver: Option<String>,
        memo: String,
    ) -> Result<String> {
        let mut chain = chain::get_chain(&self.db_pool, &chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        let rpc_client = HttpClient::new(chain.config.rpc_addr.as_str())
            .context("unable to connect to rpc client")?;

        // TODO: Remove this code once fully tested that updating solo machine client is not needed
        //
        // let mut transaction = self
        //     .db_pool
        //     .begin()
        //     .await
        //     .context("unable to begin database transaction")?;

        // let msg = transaction_builder::msg_update_solo_machine_client(
        //     &mut transaction,
        //     &signer,
        //     &mut chain,
        //     None,
        //     memo.clone(),
        // )
        // .await?;

        // let response = rpc_client
        //     .broadcast_tx_commit(proto_encode(&msg)?.into())
        //     .await?;

        // ensure_response_success(&response)?;

        // transaction
        //     .commit()
        //     .await
        //     .context("unable to commit transaction for receiving tokens over IBC")?;

        let address = signer.to_account_address()?;
        let receiver = receiver.unwrap_or_else(|| address.clone());

        let msg = transaction_builder::msg_token_receive(
            &signer,
            &chain,
            amount,
            &denom,
            receiver.clone(),
            memo.clone(),
            request_id.as_deref(),
        )
        .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        let transaction_hash = ensure_response_success(&response)?;

        operation::add_operation(
            &self.db_pool,
            request_id.as_deref(),
            &address,
            &denom,
            amount,
            &OperationType::Burn {
                chain_id: chain_id.clone(),
            },
            &transaction_hash,
        )
        .await?;

        notify_event(
            &self.notifier,
            Event::TokensBurnt {
                chain_id,
                request_id: request_id.clone(),
                from_address: address,
                amount,
                denom,
                transaction_hash: transaction_hash.clone(),
            },
        )?;

        if let Err(e) = self
            .process_packets(
                signer,
                &rpc_client,
                &mut chain,
                extract_packets(&response)?,
                memo,
                request_id,
            )
            .await
        {
            // Create a warning instead of returning an error because IBC transfer is successful even if processing of
            // packets (i.e., sending acks) fails
            notify_event(
                &self.notifier,
                Event::Warning {
                    message: e.to_string(),
                },
            )?;
        }

        Ok(transaction_hash)
    }

    /// Updates signer for future IBC transactions
    pub async fn update_signer(
        &self,
        signer: impl Signer,
        chain_id: ChainId,
        new_public_key: PublicKey,
        memo: String,
    ) -> Result<()> {
        let mut transaction = self
            .db_pool
            .begin()
            .await
            .context("unable to begin database transaction")?;

        let mut chain = chain::get_chain(&mut transaction, &chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details for {} not found", chain_id))?;

        chain_keys::add_chain_key(&mut transaction, &chain_id, &new_public_key.encode()).await?;

        let rpc_client = HttpClient::new(chain.config.rpc_addr.as_str())
            .context("unable to connect to rpc client")?;

        let msg = transaction_builder::msg_update_solo_machine_client(
            &mut transaction,
            &signer,
            &mut chain,
            Some(&new_public_key),
            memo.clone(),
        )
        .await?;

        let response = rpc_client
            .broadcast_tx_commit(proto_encode(&msg)?.into())
            .await?;

        ensure_response_success(&response)?;

        transaction
            .commit()
            .await
            .context("unable to commit transaction for receiving tokens over IBC")?;

        notify_event(
            &self.notifier,
            Event::SignerUpdated {
                chain_id,
                old_public_key: signer.to_public_key()?,
                new_public_key,
            },
        )
    }

    /// Fetches history of all operations
    pub async fn history(
        &self,
        signer: impl ToPublicKey,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Operation>> {
        let account_address = signer.to_account_address()?;
        operation::get_operations(&self.db_pool, &account_address, limit, offset).await
    }

    async fn process_packets<C>(
        &self,
        signer: impl Signer,
        rpc_client: &C,
        chain: &mut Chain,
        packets: Vec<Packet>,
        memo: String,
        request_id: Option<String>,
    ) -> Result<()>
    where
        C: Client + Send + Sync,
    {
        let connection_details = chain.connection_details.clone().ok_or_else(|| {
            anyhow!(
                "connection details for chain with id {} are missing",
                chain.id
            )
        })?;

        for packet in packets {
            ensure!(
                chain.config.port_id.to_string() == packet.source_port,
                "invalid source port id"
            );
            ensure!(
                connection_details.solo_machine_channel_id.to_string() == packet.source_channel,
                "invalid source channel id"
            );
            ensure!(
                chain.config.port_id.to_string() == packet.destination_port,
                "invalid destination port id"
            );
            ensure!(
                connection_details.tendermint_channel_id.to_string() == packet.destination_channel,
                "invalid destination channel id"
            );

            let mut transaction = self
                .db_pool
                .begin()
                .await
                .context("unable to begin database transaction")?;

            let msg = transaction_builder::msg_token_receive_ack(
                &mut *transaction,
                &signer,
                &mut *chain,
                packet,
                memo.clone(),
                request_id.as_deref(),
            )
            .await?;

            let response = rpc_client
                .broadcast_tx_commit(proto_encode(&msg)?.into())
                .await?;

            transaction
                .commit()
                .await
                .context("unable to commit transaction for processing IBC packets")?;

            ensure_response_success(&response)?;
        }

        Ok(())
    }
}

async fn create_solo_machine_client<C>(
    signer: impl Signer,
    rpc_client: &C,
    chain: &Chain,
    memo: String,
) -> Result<ClientId>
where
    C: Client + Send + Sync,
{
    let msg = transaction_builder::msg_create_solo_machine_client(signer, chain, memo).await?;

    let response = rpc_client
        .broadcast_tx_commit(proto_encode(&msg)?.into())
        .await?;

    ensure_response_success(&response)?;

    extract_attribute(&response.deliver_tx.events, "create_client", "client_id")?.parse()
}

async fn create_tendermint_client(
    transaction: &mut Transaction<'_, Db>,
    instance: &mut Instance,
    chain: &Chain,
) -> Result<ClientId> {
    let (client_state, consensus_state) =
        transaction_builder::msg_create_tendermint_client(chain, instance).await?;

    let client_id = ClientId::generate(ClientType::Tendermint);
    let latest_height = client_state
        .latest_height
        .as_ref()
        .ok_or_else(|| anyhow!("latest height cannot be absent in client state"))?;

    ibc_handler::add_tendermint_client_state(&mut *transaction, &client_id, &client_state).await?;
    ibc_handler::add_tendermint_consensus_state(
        &mut *transaction,
        &client_id,
        latest_height,
        &consensus_state,
    )
    .await?;

    Ok(client_id)
}

async fn connection_open_init<C>(
    signer: impl Signer,
    rpc_client: &C,
    chain: &Chain,
    solo_machine_client_id: &ClientId,
    tendermint_client_id: &ClientId,
    memo: String,
) -> Result<ConnectionId>
where
    C: Client + Send + Sync,
{
    let msg = transaction_builder::msg_connection_open_init(
        signer,
        chain,
        solo_machine_client_id,
        tendermint_client_id,
        memo,
    )
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

async fn connection_open_try<'e>(
    executor: impl Executor<'e, Database = Db>,
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
        state: ConnectionState::Tryopen.into(),
        delay_period: 0,
    };

    ibc_handler::add_connection(executor, &connection_id, &connection).await?;

    Ok(connection_id)
}

#[allow(clippy::too_many_arguments)]
async fn connection_open_ack<C>(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    rpc_client: &C,
    chain: &mut Chain,
    solo_machine_connection_id: &ConnectionId,
    tendermint_client_id: &ClientId,
    tendermint_connection_id: &ConnectionId,
    memo: String,
) -> Result<()>
where
    C: Client + Send + Sync,
{
    let msg = transaction_builder::msg_connection_open_ack(
        transaction,
        signer,
        chain,
        solo_machine_connection_id,
        tendermint_client_id,
        tendermint_connection_id,
        memo,
    )
    .await?;

    let response = rpc_client
        .broadcast_tx_commit(proto_encode(&msg)?.into())
        .await?;

    ensure_response_success(&response)?;

    Ok(())
}

async fn connection_open_confirm(
    transaction: &mut Transaction<'_, Db>,
    connection_id: &ConnectionId,
) -> Result<()> {
    let mut connection = ibc_handler::get_connection(&mut *transaction, connection_id)
        .await?
        .ok_or_else(|| anyhow!("connection for connection id ({}) not found", connection_id))?;
    connection.set_state(ConnectionState::Open);

    ibc_handler::update_connection(&mut *transaction, connection_id, &connection).await
}

async fn channel_open_init<C>(
    signer: impl Signer,
    rpc_client: &C,
    chain: &Chain,
    solo_machine_connection_id: &ConnectionId,
    memo: String,
) -> Result<ChannelId>
where
    C: Client + Send + Sync,
{
    let msg =
        transaction_builder::msg_channel_open_init(signer, chain, solo_machine_connection_id, memo)
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

async fn channel_open_try<'e>(
    executor: impl Executor<'e, Database = Db>,
    port_id: &PortId,
    solo_machine_channel_id: &ChannelId,
    tendermint_connection_id: &ConnectionId,
) -> Result<ChannelId> {
    let channel_id = ChannelId::generate();

    let channel = Channel {
        state: ChannelState::Tryopen.into(),
        ordering: ChannelOrder::Unordered.into(),
        counterparty: Some(ChannelCounterparty {
            port_id: port_id.to_string(),
            channel_id: solo_machine_channel_id.to_string(),
        }),
        connection_hops: vec![tendermint_connection_id.to_string()],
        version: "ics20-1".to_string(),
    };

    ibc_handler::add_channel(executor, port_id, &channel_id, &channel).await?;

    Ok(channel_id)
}

async fn channel_open_ack<C>(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    rpc_client: &C,
    chain: &mut Chain,
    solo_machine_channel_id: &ChannelId,
    tendermint_channel_id: &ChannelId,
    memo: String,
) -> Result<()>
where
    C: Client + Send + Sync,
{
    let msg = transaction_builder::msg_channel_open_ack(
        transaction,
        signer,
        chain,
        solo_machine_channel_id,
        tendermint_channel_id,
        memo,
    )
    .await?;

    let response = rpc_client
        .broadcast_tx_commit(proto_encode(&msg)?.into())
        .await?;

    ensure_response_success(&response)?;

    Ok(())
}

async fn channel_open_confirm(
    transaction: &mut Transaction<'_, Db>,
    port_id: &PortId,
    channel_id: &ChannelId,
) -> Result<()> {
    let mut channel = ibc_handler::get_channel(&mut *transaction, port_id, channel_id)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "channel for channel id ({}) and port id ({}) not found",
                channel_id,
                port_id
            )
        })?;
    channel.set_state(ChannelState::Open);

    ibc_handler::update_channel(&mut *transaction, port_id, channel_id, &channel).await
}

fn prepare_light_client(
    chain: &Chain,
    rpc_client: HttpClient,
    light_store: Box<dyn LightStore>,
) -> Result<Instance> {
    let builder = LightClientBuilder::prod(
        chain.node_id,
        rpc_client,
        light_store,
        Options {
            trust_threshold: TrustThresholdFraction::new(
                *chain.config.trust_level.numer(),
                *chain.config.trust_level.denom(),
            )
            .unwrap(),
            trusting_period: chain.config.trusting_period,
            clock_drift: chain.config.max_clock_drift,
        },
        Some(chain.config.rpc_timeout),
    );

    let builder = builder.trust_primary_at(
        chain.config.trusted_height,
        TendermintHash::Sha256(chain.config.trusted_hash),
    )?;

    Ok(builder.build())
}

fn extract_packets(response: &TxCommitResponse) -> Result<Vec<Packet>> {
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

fn ensure_response_success(response: &TxCommitResponse) -> Result<String> {
    ensure!(
        response.check_tx.code.is_ok(),
        "check_tx response contains error code: {}",
        response.check_tx.log
    );

    ensure!(
        response.deliver_tx.code.is_ok(),
        "deliver_tx response contains error code: {}",
        response.deliver_tx.log
    );

    Ok(response.hash.to_string())
}

fn extract_attribute(events: &[AbciEvent], event_type: &str, key: &str) -> Result<String> {
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

fn get_attribute(tags: &[Tag], key: &str) -> Result<String> {
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
