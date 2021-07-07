use std::convert::TryInto;

#[cfg(feature = "solomachine-v2")]
use crate::proto::ibc::lightclients::solomachine::v2::{
    ChannelStateData, ClientState as SoloMachineClientState, ClientStateData, ConnectionStateData,
    ConsensusState as SoloMachineConsensusState, ConsensusStateData, DataType,
    Header as SoloMachineHeader, HeaderData, PacketAcknowledgementData, PacketCommitmentData,
    SignBytes, TimestampedSignatureData,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use chrono::{DateTime, Utc};
#[cfg(not(feature = "solomachine-v2"))]
use cosmos_sdk_proto::ibc::lightclients::solomachine::v1::{
    ChannelStateData, ClientState as SoloMachineClientState, ClientStateData, ConnectionStateData,
    ConsensusState as SoloMachineConsensusState, ConsensusStateData, DataType,
    Header as SoloMachineHeader, HeaderData, PacketAcknowledgementData, PacketCommitmentData,
    SignBytes, TimestampedSignatureData,
};
use cosmos_sdk_proto::{
    cosmos::{
        auth::v1beta1::{query_client::QueryClient as AuthQueryClient, QueryAccountRequest},
        base::v1beta1::Coin,
        staking::v1beta1::{query_client::QueryClient as StakingQueryClient, QueryParamsRequest},
        tx::{
            signing::v1beta1::{
                signature_descriptor::{
                    data::{Single as SingleSignatureData, Sum as SignatureDataInner},
                    Data as SignatureData,
                },
                SignMode,
            },
            v1beta1::{
                mode_info::{Single, Sum},
                AuthInfo, Fee, ModeInfo, SignDoc, SignerInfo, TxBody, TxRaw,
            },
        },
    },
    ibc::{
        applications::transfer::v1::MsgTransfer,
        core::{
            channel::v1::{
                Channel, Counterparty as ChannelCounterparty, MsgAcknowledgement,
                MsgChannelOpenAck, MsgChannelOpenInit, MsgRecvPacket, Order as ChannelOrder,
                Packet, State as ChannelState,
            },
            client::v1::{Height, MsgCreateClient, MsgUpdateClient},
            commitment::v1::MerklePrefix,
            connection::v1::{
                Counterparty as ConnectionCounterparty, MsgConnectionOpenAck,
                MsgConnectionOpenInit, Version as ConnectionVersion,
            },
        },
        lightclients::tendermint::v1::{
            ClientState as TendermintClientState, ConsensusState as TendermintConsensusState,
            Fraction,
        },
    },
};
use prost_types::{Any, Duration};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, Transaction};
use tendermint::block::Header;
use tendermint_light_client::supervisor::Instance;
use tendermint_rpc::Client;

use crate::{
    cosmos::account::Account,
    ibc::{
        client::ics07_tendermint::consensus_state::IConsensusState,
        core::{
            ics02_client::height::IHeight,
            ics04_channel::packet::IPacket,
            ics23_vector_commitments::proof_specs,
            ics24_host::{
                identifier::{ChainId, ChannelId, ClientId, ConnectionId, Identifier},
                path::{
                    ChannelPath, ClientStatePath, ConnectionPath, ConsensusStatePath,
                    PacketAcknowledgementPath, PacketCommitmentPath,
                },
            },
        },
    },
    model::{chain, ibc as ibc_handler, Chain},
    proto::{proto_encode, AnyConvert},
    Db, Signer, ToPublicKey,
};

const DEFAULT_TIMEOUT_HEIGHT_OFFSET: u64 = 10;

/// Builds a transaction to create a solo machine client on IBC enabled chain
pub async fn msg_create_solo_machine_client(
    signer: impl Signer,
    chain: &Chain,
    memo: String,
) -> Result<TxRaw> {
    let any_public_key = signer.to_public_key()?.to_any()?;

    let consensus_state = SoloMachineConsensusState {
        public_key: Some(any_public_key),
        diversifier: chain.config.diversifier.clone(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
    };
    let any_consensus_state = consensus_state.to_any()?;

    let client_state = SoloMachineClientState {
        sequence: chain.sequence.into(),
        #[cfg(not(feature = "solomachine-v2"))]
        frozen_sequence: 0,
        #[cfg(feature = "solomachine-v2")]
        is_frozen: false,
        consensus_state: Some(consensus_state),
        allow_update_after_proposal: true,
    };
    let any_client_state = client_state.to_any()?;

    let message = MsgCreateClient {
        client_state: Some(any_client_state),
        consensus_state: Some(any_consensus_state),
        signer: signer.to_account_address()?,
    };

    build(signer, chain, &[message], memo).await
}

/// Builds a transaction to update solo machine client on IBC enabled chain
pub async fn msg_update_solo_machine_client<'e>(
    executor: impl Executor<'e, Database = Db>,
    signer: impl Signer,
    chain: &mut Chain,
    memo: String,
) -> Result<TxRaw> {
    if chain.connection_details.is_none() {
        bail!(
            "connection details not found for chain with id {}",
            chain.id
        );
    }

    let any_public_key = signer.to_public_key()?.to_any()?;

    let sequence = chain.sequence.into();

    let signature = get_header_proof(
        &signer,
        &chain,
        Some(any_public_key.clone()),
        chain.config.diversifier.clone(),
    )?;

    *chain = chain::increment_sequence(executor, &chain.id).await?;

    let header = SoloMachineHeader {
        sequence,
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        signature,
        new_public_key: Some(any_public_key),
        new_diversifier: chain.config.diversifier.clone(),
    };

    let any_header = header.to_any()?;

    let connection_details = chain.connection_details.as_ref().ok_or_else(|| {
        anyhow!(
            "connection details not found for chain with id {}",
            chain.id
        )
    })?;

    let message = MsgUpdateClient {
        client_id: connection_details.solo_machine_client_id.to_string(),
        header: Some(any_header),
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

/// Builds a transaction to create a tendermint client on IBC enabled solo machine
pub async fn msg_create_tendermint_client(
    chain: &Chain,
    instance: &mut Instance,
) -> Result<(TendermintClientState, TendermintConsensusState)> {
    let trust_level = Some(Fraction {
        numerator: *chain.config.trust_level.numer(),
        denominator: *chain.config.trust_level.denom(),
    });

    let unbonding_period = Some(get_unbonding_period(&chain).await?);
    let latest_header = get_latest_header(instance)?;
    let latest_height = get_block_height(&chain, &latest_header);

    let client_state = TendermintClientState {
        chain_id: chain.id.to_string(),
        trust_level,
        trusting_period: Some(chain.config.trusting_period.into()),
        unbonding_period,
        max_clock_drift: Some(chain.config.max_clock_drift.into()),
        frozen_height: Some(Height::zero()),
        latest_height: Some(latest_height),
        proof_specs: proof_specs(),
        upgrade_path: vec!["upgrade".to_string(), "upgradedIBCState".to_string()],
        allow_update_after_expiry: false,
        allow_update_after_misbehaviour: false,
    };

    let consensus_state = TendermintConsensusState::from_block_header(latest_header);

    Ok((client_state, consensus_state))
}

pub async fn msg_connection_open_init(
    signer: impl Signer,
    chain: &Chain,
    solo_machine_client_id: &ClientId,
    tendermint_client_id: &ClientId,
    memo: String,
) -> Result<TxRaw> {
    let message = MsgConnectionOpenInit {
        client_id: solo_machine_client_id.to_string(),
        counterparty: Some(ConnectionCounterparty {
            client_id: tendermint_client_id.to_string(),
            connection_id: "".to_string(),
            prefix: Some(MerklePrefix {
                key_prefix: "ibc".as_bytes().to_vec(),
            }),
        }),
        version: Some(ConnectionVersion {
            identifier: "1".to_string(),
            features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
        }),
        delay_period: 0,
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

pub async fn msg_connection_open_ack(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    chain: &mut Chain,
    solo_machine_connection_id: &ConnectionId,
    tendermint_client_id: &ClientId,
    tendermint_connection_id: &ConnectionId,
    memo: String,
) -> Result<TxRaw> {
    let tendermint_client_state =
        ibc_handler::get_tendermint_client_state(&mut *transaction, tendermint_client_id)
            .await?
            .ok_or_else(|| anyhow!("client for client id {} not found", tendermint_client_id))?;

    let proof_height = Height::new(0, chain.sequence.into());

    let proof_try =
        get_connection_proof(&mut *transaction, &signer, &chain, tendermint_connection_id).await?;
    *chain = chain::increment_sequence(&mut *transaction, &chain.id).await?;

    let proof_client =
        get_client_proof(&mut *transaction, &signer, &chain, &tendermint_client_id).await?;
    *chain = chain::increment_sequence(&mut *transaction, &chain.id).await?;

    let proof_consensus =
        get_consensus_proof(&mut *transaction, &signer, &chain, &tendermint_client_id).await?;
    *chain = chain::increment_sequence(&mut *transaction, &chain.id).await?;

    let message = MsgConnectionOpenAck {
        connection_id: solo_machine_connection_id.to_string(),
        counterparty_connection_id: tendermint_connection_id.to_string(),
        version: Some(ConnectionVersion {
            identifier: "1".to_string(),
            features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
        }),
        client_state: Some(tendermint_client_state.to_any()?),
        proof_height: Some(proof_height),
        proof_try,
        proof_client,
        proof_consensus,
        consensus_height: tendermint_client_state.latest_height,
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

pub async fn msg_channel_open_init(
    signer: impl Signer,
    chain: &Chain,
    solo_machine_connection_id: &ConnectionId,
    memo: String,
) -> Result<TxRaw> {
    let message = MsgChannelOpenInit {
        port_id: chain.config.port_id.to_string(),
        channel: Some(Channel {
            state: ChannelState::Init.into(),
            ordering: ChannelOrder::Unordered.into(),
            counterparty: Some(ChannelCounterparty {
                port_id: chain.config.port_id.to_string(),
                channel_id: "".to_string(),
            }),
            connection_hops: vec![solo_machine_connection_id.to_string()],
            version: "ics20-1".to_string(),
        }),
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

pub async fn msg_channel_open_ack(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    chain: &mut Chain,
    solo_machine_channel_id: &ChannelId,
    tendermint_channel_id: &ChannelId,
    memo: String,
) -> Result<TxRaw> {
    let proof_height = Height::new(0, chain.sequence.into());

    let proof_try =
        get_channel_proof(&mut *transaction, &signer, &chain, tendermint_channel_id).await?;
    *chain = chain::increment_sequence(&mut *transaction, &chain.id).await?;

    let message = MsgChannelOpenAck {
        port_id: chain.config.port_id.to_string(),
        channel_id: solo_machine_channel_id.to_string(),
        counterparty_channel_id: tendermint_channel_id.to_string(),
        counterparty_version: "ics20-1".to_string(),
        proof_height: Some(proof_height),
        proof_try,
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

#[allow(clippy::too_many_arguments)]
pub async fn msg_token_send<C>(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    rpc_client: &C,
    chain: &mut Chain,
    amount: u32,
    denom: &Identifier,
    receiver: String,
    memo: String,
) -> Result<TxRaw>
where
    C: Client + Send + Sync,
{
    let connection_details = chain.connection_details.as_ref().ok_or_else(|| {
        anyhow!(
            "connection details not found for chain with id {}",
            chain.id
        )
    })?;

    let sender = signer.to_account_address()?;

    let packet_data = TokenTransferPacketData {
        denom: denom.to_string(),
        amount: amount.into(),
        sender: sender.clone(),
        receiver,
    };

    let packet = Packet {
        sequence: chain.packet_sequence.into(),
        source_port: chain.config.port_id.to_string(),
        source_channel: connection_details.tendermint_channel_id.to_string(),
        destination_port: chain.config.port_id.to_string(),
        destination_channel: connection_details.solo_machine_channel_id.to_string(),
        data: serde_json::to_vec(&packet_data)?,
        timeout_height: Some(
            get_latest_height(&chain, rpc_client)
                .await?
                .checked_add(DEFAULT_TIMEOUT_HEIGHT_OFFSET)
                .ok_or_else(|| anyhow!("height addition overflow"))?,
        ),
        timeout_timestamp: 0,
    };

    let proof_commitment = get_packet_commitment_proof(&signer, &chain, &packet)?;

    let proof_height = Height::new(0, chain.sequence.into());

    *chain = chain::increment_sequence(&mut *transaction, &chain.id).await?;
    *chain = chain::increment_packet_sequence(&mut *transaction, &chain.id).await?;

    let message = MsgRecvPacket {
        packet: Some(packet),
        proof_commitment,
        proof_height: Some(proof_height),
        signer: sender,
    };

    build(signer, &chain, &[message], memo).await
}

pub async fn msg_token_receive(
    signer: impl Signer,
    chain: &Chain,
    amount: u32,
    denom: &Identifier,
    receiver: String,
    memo: String,
) -> Result<TxRaw> {
    let connection_details = chain.connection_details.as_ref().ok_or_else(|| {
        anyhow!(
            "connection details not found for chain with id {}",
            chain.id
        )
    })?;

    let denom = chain.get_ibc_denom(denom).ok_or_else(|| {
        anyhow!(
            "connection is not established with chain with id {}",
            chain.id
        )
    })?;

    let sender = signer.to_account_address()?;

    let message = MsgTransfer {
        source_port: chain.config.port_id.to_string(),
        source_channel: connection_details.solo_machine_channel_id.to_string(),
        token: Some(Coin {
            amount: amount.to_string(),
            denom,
        }),
        sender,
        receiver,
        timeout_height: Some(Height::new(0, u64::from(chain.sequence) + 1)),
        timeout_timestamp: 0,
    };

    build(signer, &chain, &[message], memo).await
}

pub async fn msg_token_receive_ack<'e>(
    executor: impl Executor<'e, Database = Db>,
    signer: impl Signer,
    chain: &mut Chain,
    packet: Packet,
    memo: String,
) -> Result<TxRaw> {
    let proof_height = Height::new(0, chain.sequence.into());
    let acknowledgement = serde_json::to_vec(&json!({ "result": [1] }))?;

    let proof_acked = get_packet_acknowledgement_proof(
        &signer,
        &chain,
        acknowledgement.clone(),
        packet.sequence,
    )?;

    *chain = chain::increment_sequence(executor, &chain.id).await?;

    let message = MsgAcknowledgement {
        packet: Some(packet),
        acknowledgement,
        proof_acked,
        proof_height: Some(proof_height),
        signer: signer.to_account_address()?,
    };

    build(signer, &chain, &[message], memo).await
}

async fn build<T>(signer: impl Signer, chain: &Chain, messages: &[T], memo: String) -> Result<TxRaw>
where
    T: AnyConvert,
{
    let tx_body = build_tx_body(messages, memo).context("unable to build transaction body")?;
    let tx_body_bytes = proto_encode(&tx_body)?;

    let (account_number, account_sequence) = get_account_details(&signer, &chain).await?;

    let auth_info =
        build_auth_info(&signer, &chain, account_sequence).context("unable to build auth info")?;
    let auth_info_bytes = proto_encode(&auth_info)?;

    let signature = build_signature(
        signer,
        tx_body_bytes.clone(),
        auth_info_bytes.clone(),
        chain.id.to_string(),
        account_number,
    )
    .context("unable to sign transaction")?;

    Ok(TxRaw {
        body_bytes: tx_body_bytes,
        auth_info_bytes,
        signatures: vec![signature],
    })
}

fn build_tx_body<T>(messages: &[T], memo: String) -> Result<TxBody>
where
    T: AnyConvert,
{
    let messages = messages
        .iter()
        .map(AnyConvert::to_any)
        .collect::<Result<_, _>>()?;

    Ok(TxBody {
        messages,
        memo,
        timeout_height: 0,
        extension_options: Default::default(),
        non_critical_extension_options: Default::default(),
    })
}

fn build_auth_info(
    signer: impl ToPublicKey,
    chain: &Chain,
    account_sequence: u64,
) -> Result<AuthInfo> {
    let signer_info = SignerInfo {
        public_key: Some(signer.to_public_key()?.to_any()?),
        mode_info: Some(ModeInfo {
            sum: Some(Sum::Single(Single { mode: 1 })),
        }),
        sequence: account_sequence,
    };

    let fee = Fee {
        amount: vec![Coin {
            denom: chain.config.fee.denom.to_string(),
            amount: chain.config.fee.amount.to_string(),
        }],
        gas_limit: chain.config.fee.gas_limit,
        payer: "".to_owned(),
        granter: "".to_owned(),
    };

    Ok(AuthInfo {
        signer_infos: vec![signer_info],
        fee: Some(fee),
    })
}

fn build_signature(
    signer: impl Signer,
    body_bytes: Vec<u8>,
    auth_info_bytes: Vec<u8>,
    chain_id: String,
    account_number: u64,
) -> Result<Vec<u8>> {
    let sign_doc = SignDoc {
        body_bytes,
        auth_info_bytes,
        chain_id,
        account_number,
    };

    let sign_doc_bytes = proto_encode(&sign_doc)?;

    signer.sign(&sign_doc_bytes)
}

async fn get_account_details(signer: impl ToPublicKey, chain: &Chain) -> Result<(u64, u64)> {
    let mut query_client = AuthQueryClient::connect(chain.config.grpc_addr.clone())
        .await
        .context(format!(
            "unable to connect to grpc query client at {}",
            chain.config.grpc_addr
        ))?;

    let account_address = signer.to_account_address()?;

    let response = query_client
        .account(QueryAccountRequest {
            address: account_address.clone(),
        })
        .await?
        .into_inner()
        .account
        .ok_or_else(|| anyhow!("unable to find account with address: {}", account_address))?;

    let account = Account::from_any(&response)?;
    let base_account = account
        .get_base_account()
        .ok_or_else(|| anyhow!("missing base account for address: {}", account_address))?;

    Ok((base_account.account_number, base_account.sequence))
}

async fn get_unbonding_period(chain: &Chain) -> Result<Duration> {
    let mut query_client = StakingQueryClient::connect(chain.config.grpc_addr.clone())
        .await
        .context(format!(
            "unable to connect to grpc query client at {}",
            chain.config.grpc_addr
        ))?;

    query_client
        .params(QueryParamsRequest::default())
        .await?
        .into_inner()
        .params
        .ok_or_else(|| anyhow!("staking params are empty"))?
        .unbonding_time
        .ok_or_else(|| anyhow!("missing unbonding period in staking params"))
}

async fn get_latest_height<C>(chain: &Chain, rpc_client: &C) -> Result<Height>
where
    C: Client + Send + Sync,
{
    let response = rpc_client.status().await?;

    ensure!(
        !response.sync_info.catching_up,
        "node at {} running chain {} not caught up",
        chain.config.rpc_addr,
        chain.id,
    );

    let revision_number = response
        .node_info
        .network
        .as_str()
        .parse::<ChainId>()?
        .version();

    let revision_height = response.sync_info.latest_block_height.into();

    Ok(Height {
        revision_number,
        revision_height,
    })
}

fn get_latest_header(instance: &mut Instance) -> Result<Header> {
    let light_block = instance
        .light_client
        .verify_to_highest(&mut instance.state)?;

    Ok(light_block.signed_header.header)
}

fn get_block_height(chain: &Chain, header: &Header) -> Height {
    let revision_number = chain.id.version();
    let revision_height = header.height.value();

    Height {
        revision_number,
        revision_height,
    }
}

fn get_packet_acknowledgement_proof(
    signer: impl Signer,
    chain: &Chain,
    acknowledgement: Vec<u8>,
    packet_sequence: u64,
) -> Result<Vec<u8>> {
    let mut acknowledgement_path = PacketAcknowledgementPath::new(
        &chain.config.port_id,
        &chain
            .connection_details
            .as_ref()
            .ok_or_else(|| {
                anyhow!(
                    "connection details for chain with id {} not found",
                    chain.id
                )
            })?
            .tendermint_channel_id,
        packet_sequence,
    );
    acknowledgement_path.apply_prefix(&"ibc".parse().unwrap());

    let acknowledgement_data = PacketAcknowledgementData {
        path: acknowledgement_path.into_bytes(),
        acknowledgement,
    };

    let acknowledgement_data_bytes = proto_encode(&acknowledgement_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::PacketAcknowledgement.into(),
        data: acknowledgement_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

fn get_packet_commitment_proof(
    signer: impl Signer,
    chain: &Chain,
    packet: &Packet,
) -> Result<Vec<u8>> {
    let commitment_bytes = packet.commitment_bytes()?;

    let mut commitment_path = PacketCommitmentPath::new(
        &chain.config.port_id,
        &chain
            .connection_details
            .as_ref()
            .ok_or_else(|| {
                anyhow!(
                    "connection details for chain with id {} not found",
                    chain.id
                )
            })?
            .tendermint_channel_id,
        chain.packet_sequence.into(),
    );
    commitment_path.apply_prefix(&"ibc".parse().unwrap());

    let packet_commitment_data = PacketCommitmentData {
        path: commitment_path.into_bytes(),
        commitment: commitment_bytes,
    };

    let packet_commitment_data_bytes = proto_encode(&packet_commitment_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::PacketCommitment.into(),
        data: packet_commitment_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

async fn get_channel_proof<'e>(
    executor: impl Executor<'e, Database = Db>,
    signer: impl Signer,
    chain: &Chain,
    channel_id: &ChannelId,
) -> Result<Vec<u8>> {
    let channel = ibc_handler::get_channel(executor, &chain.config.port_id, channel_id)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "channel with port id {} and channel id {} not found",
                chain.config.port_id,
                channel_id
            )
        })?;

    let mut channel_path = ChannelPath::new(&chain.config.port_id, channel_id);
    channel_path.apply_prefix(&"ibc".parse().unwrap());

    let channel_state_data = ChannelStateData {
        path: channel_path.into_bytes(),
        channel: Some(channel),
    };

    let channel_state_data_bytes = proto_encode(&channel_state_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::ChannelState.into(),
        data: channel_state_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

async fn get_connection_proof<'e>(
    executor: impl Executor<'e, Database = Db>,
    signer: impl Signer,
    chain: &Chain,
    connection_id: &ConnectionId,
) -> Result<Vec<u8>> {
    let connection = ibc_handler::get_connection(executor, connection_id)
        .await?
        .ok_or_else(|| anyhow!("connection with id {} not found", connection_id))?;

    let mut connection_path = ConnectionPath::new(connection_id);
    connection_path.apply_prefix(&"ibc".parse().unwrap());

    let connection_state_data = ConnectionStateData {
        path: connection_path.into_bytes(),
        connection: Some(connection),
    };

    let connection_state_data_bytes = proto_encode(&connection_state_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::ConnectionState.into(),
        data: connection_state_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

async fn get_client_proof<'e>(
    executor: impl Executor<'e, Database = Db>,
    signer: impl Signer,
    chain: &Chain,
    client_id: &ClientId,
) -> Result<Vec<u8>> {
    let client_state = ibc_handler::get_tendermint_client_state(executor, client_id)
        .await?
        .ok_or_else(|| anyhow!("client with id {} not found", client_id))?
        .to_any()?;

    let mut client_state_path = ClientStatePath::new(client_id);
    client_state_path.apply_prefix(&"ibc".parse().unwrap());

    let client_state_data = ClientStateData {
        path: client_state_path.into_bytes(),
        client_state: Some(client_state),
    };

    let client_state_data_bytes = proto_encode(&client_state_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::ClientState.into(),
        data: client_state_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

async fn get_consensus_proof(
    transaction: &mut Transaction<'_, Db>,
    signer: impl Signer,
    chain: &Chain,
    client_id: &ClientId,
) -> Result<Vec<u8>> {
    let client_state = ibc_handler::get_tendermint_client_state(&mut *transaction, client_id)
        .await?
        .ok_or_else(|| anyhow!("client with id {} not found", client_id))?;

    let height = client_state
        .latest_height
        .ok_or_else(|| anyhow!("client state does not contain latest height"))?;

    let consensus_state =
        ibc_handler::get_tendermint_consensus_state(&mut *transaction, client_id, &height)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "consensus state with id {} and height {} not found",
                    client_id,
                    height.to_string(),
                )
            })?
            .to_any()?;

    let mut consensus_state_path = ConsensusStatePath::new(client_id, &height);
    consensus_state_path.apply_prefix(&"ibc".parse().unwrap());

    let consensus_state_data = ConsensusStateData {
        path: consensus_state_path.into_bytes(),
        consensus_state: Some(consensus_state),
    };

    let consensus_state_data_bytes = proto_encode(&consensus_state_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::ConsensusState.into(),
        data: consensus_state_data_bytes,
    };

    timestamped_sign(signer, chain, sign_bytes)
}

fn get_header_proof(
    signer: impl Signer,
    chain: &Chain,
    new_public_key: Option<Any>,
    new_diversifier: String,
) -> Result<Vec<u8>> {
    let header_data = HeaderData {
        new_pub_key: new_public_key,
        new_diversifier,
    };

    let header_data_bytes = proto_encode(&header_data)?;

    let sign_bytes = SignBytes {
        sequence: chain.sequence.into(),
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
        diversifier: chain.config.diversifier.to_owned(),
        data_type: DataType::Header.into(),
        data: header_data_bytes,
    };

    sign(signer, sign_bytes)
}

fn timestamped_sign(signer: impl Signer, chain: &Chain, sign_bytes: SignBytes) -> Result<Vec<u8>> {
    let signature_data = sign(signer, sign_bytes)?;

    let timestamped_signature_data = TimestampedSignatureData {
        signature_data,
        timestamp: to_u64_timestamp(chain.consensus_timestamp)?,
    };

    proto_encode(&timestamped_signature_data)
}

fn sign(signer: impl Signer, sign_bytes: SignBytes) -> Result<Vec<u8>> {
    let sign_bytes = proto_encode(&sign_bytes)?;
    let signature = signer.sign(&sign_bytes)?;

    let signature_data = SignatureData {
        sum: Some(SignatureDataInner::Single(SingleSignatureData {
            signature,
            mode: SignMode::Unspecified.into(),
        })),
    };

    proto_encode(&signature_data)
}

fn to_u64_timestamp(timestamp: DateTime<Utc>) -> Result<u64> {
    timestamp
        .timestamp()
        .try_into()
        .context("unable to convert unix timestamp to u64")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenTransferPacketData {
    pub denom: String,
    pub amount: u64,
    pub sender: String,
    pub receiver: String,
}
