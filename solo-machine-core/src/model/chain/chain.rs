use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use anyhow::{anyhow, ensure, Context, Error, Result};
use chrono::{DateTime, Utc};
use cosmos_sdk_proto::cosmos::bank::v1beta1::{
    query_client::QueryClient as BankQueryClient, QueryBalanceRequest,
};
use num_rational::Ratio;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{types::Json, Executor, FromRow};
use tendermint::{block::Height as BlockHeight, node::Id as NodeId};

use crate::{
    ibc::core::ics24_host::{
        identifier::{ChainId, ChannelId, ClientId, ConnectionId, Identifier, PortId},
        path::DenomTrace,
    },
    Db, ToPublicKey,
};

/// State of an IBC enabled chain
#[derive(Debug, Serialize, Deserialize)]
pub struct Chain {
    /// ID of chain
    pub id: ChainId,
    /// Node ID of chain
    pub node_id: NodeId,
    /// Configuration for chain
    pub config: ChainConfig,
    /// Consensus timestamp of solo machine (used when creating transactions on chain)
    pub consensus_timestamp: DateTime<Utc>,
    /// Sequence of solo machine (used when creating transactions on chain)
    pub sequence: u32,
    /// Packet sequence of solo machine (used when creating transactions on chain)
    pub packet_sequence: u32,
    /// IBC connection details
    pub connection_details: Option<ConnectionDetails>,
    /// Creation time of chain
    pub created_at: DateTime<Utc>,
    /// Last updation time of chain
    pub updated_at: DateTime<Utc>,
}

impl Chain {
    /// Returns the IBC denom of given denomination based on connection details. Returns `None` if connection details
    /// are not present.
    pub fn get_ibc_denom(&self, denom: &Identifier) -> Option<String> {
        let connection_details = self.connection_details.as_ref()?;

        let denom_trace = DenomTrace::new(
            &self.config.port_id,
            &connection_details.solo_machine_channel_id,
            denom,
        );

        let hash = Sha256::digest(denom_trace.to_string().as_bytes());

        Some(format!("ibc/{}", hex::encode_upper(hash)))
    }

    /// Fetches on-chain balance of given denom
    pub async fn get_balance(
        &self,
        signer: impl ToPublicKey,
        denom: &Identifier,
    ) -> Result<Decimal> {
        let mut query_client = BankQueryClient::connect(self.config.grpc_addr.clone())
            .await
            .context(format!(
                "unable to connect to grpc query client at {}",
                self.config.grpc_addr
            ))?;

        let denom = self
            .get_ibc_denom(denom)
            .ok_or_else(|| anyhow!("connection details not found when fetching balance"))?;

        let request = QueryBalanceRequest {
            address: signer.to_account_address()?,
            denom,
        };

        Ok(query_client
            .balance(request)
            .await?
            .into_inner()
            .balance
            .map(|coin| coin.amount.parse())
            .transpose()?
            .unwrap_or_default())
    }
}

#[derive(Debug, FromRow)]
/// Raw state of an IBC enabled chain
struct RawChain {
    /// ID of chain
    pub id: String,
    /// Node ID of chain
    pub node_id: String,
    /// Configuration for chain
    pub config: Json<ChainConfig>,
    /// Consensus timestamp of solo machine (used when creating transactions on chain)
    pub consensus_timestamp: DateTime<Utc>,
    /// Sequence of solo machine (used when creating transactions on chain)
    pub sequence: i64,
    /// Packet sequence of solo machine (used when creating transactions on chain)
    pub packet_sequence: i64,
    /// IBC connection details
    pub connection_details: Option<Json<ConnectionDetails>>,
    /// Creation time of chain
    pub created_at: DateTime<Utc>,
    /// Last updation time of chain
    pub updated_at: DateTime<Utc>,
}

/// Configuration related to an IBC enabled chain
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainConfig {
    /// gRPC address
    pub grpc_addr: String,
    /// RPC address
    pub rpc_addr: String,
    /// Fee and gas limits
    pub fee: Fee,
    /// Trust level (e.g. 1/3)
    pub trust_level: Ratio<u64>,
    /// Trusting period
    pub trusting_period: Duration,
    /// Maximum clock drift
    pub max_clock_drift: Duration,
    /// RPC timeout duration
    pub rpc_timeout: Duration,
    /// Diversifier used in transactions for chain
    pub diversifier: String,
    /// Port ID used to create connection with chain
    pub port_id: PortId,
    /// Trusted height of the chain
    pub trusted_height: BlockHeight,
    /// Block hash at trusted height of the chain
    #[serde(with = "hex::serde")]
    pub trusted_hash: [u8; 32],
}

/// Fee and gas configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct Fee {
    /// Fee amount
    pub amount: Decimal,
    /// Denom of fee
    pub denom: Identifier,
    /// Gas limit
    pub gas_limit: u64,
}

/// IBC connection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDetails {
    /// Client ID of solo machine client on IBC enabled chain
    pub solo_machine_client_id: ClientId,
    /// Client ID of IBC enabled chain on solo machine
    pub tendermint_client_id: ClientId,
    /// Connection ID of solo machine client on IBC enabled chain
    pub solo_machine_connection_id: ConnectionId,
    /// Connection ID of IBC enabled chain on solo machine
    pub tendermint_connection_id: ConnectionId,
    /// Channel ID of solo machine client on IBC enabled chain
    pub solo_machine_channel_id: ChannelId,
    /// Channel ID of IBC enabled chain on solo machine
    pub tendermint_channel_id: ChannelId,
}

impl From<Chain> for RawChain {
    fn from(chain: Chain) -> Self {
        Self {
            id: chain.id.to_string(),
            node_id: chain.node_id.to_string(),
            config: Json(chain.config),
            consensus_timestamp: chain.consensus_timestamp,
            sequence: chain.sequence.into(),
            packet_sequence: chain.packet_sequence.into(),
            connection_details: chain.connection_details.map(Json),
            created_at: chain.created_at,
            updated_at: chain.updated_at,
        }
    }
}

impl TryFrom<RawChain> for Chain {
    type Error = Error;

    fn try_from(raw: RawChain) -> Result<Self, Self::Error> {
        Ok(Self {
            id: raw.id.parse()?,
            node_id: raw
                .node_id
                .parse()
                .map_err(|err| anyhow!("unable to parse node id: {}", err))?,
            config: raw.config.0,
            consensus_timestamp: raw.consensus_timestamp,
            sequence: raw.sequence.try_into()?,
            packet_sequence: raw.packet_sequence.try_into()?,
            connection_details: raw.connection_details.map(|json| json.0),
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

/// Adds a chain to database
pub async fn add_chain<'e>(
    executor: impl Executor<'e, Database = Db>,
    id: &ChainId,
    node_id: &NodeId,
    config: &ChainConfig,
) -> Result<()> {
    let id = id.to_string();
    let node_id = node_id.to_string();
    let config = Json(config);

    let rows_affected = sqlx::query("INSERT INTO chains (id, node_id, config) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(node_id)
        .bind(config)
        .execute(executor)
        .await
        .context("unable to add chain details in database")?
        .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding new chain details"
    );

    Ok(())
}

/// Fetches chain from database
pub async fn get_chain<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
) -> Result<Option<Chain>> {
    sqlx::query_as("SELECT * FROM chains WHERE id = $1")
        .bind(chain_id.to_string())
        .fetch_optional(executor)
        .await
        .context("unable to query chain from database")?
        .map(|raw: RawChain| raw.try_into())
        .transpose()
}

/// Adds connection details for given chain id
pub async fn add_connection_details<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
    connection_details: &ConnectionDetails,
) -> Result<()> {
    let connection_details = Json(connection_details);

    let rows_affected =
        sqlx::query("UPDATE chains SET connection_details = $1, updated_at = $2 WHERE id = $3")
            .bind(connection_details)
            .bind(Utc::now())
            .bind(chain_id.to_string())
            .execute(executor)
            .await
            .context("unable to add connection details to chain")?
            .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding connection details to chain"
    );

    Ok(())
}

pub async fn increment_sequence<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
) -> Result<Chain> {
    let raw: RawChain = sqlx::query_as(
        "UPDATE chains SET sequence = sequence + 1, updated_at = $1 WHERE id = $2 RETURNING *",
    )
    .bind(Utc::now())
    .bind(chain_id.to_string())
    .fetch_one(executor)
    .await
    .context("unable to increment sequence of a chain")?;

    raw.try_into()
}

pub async fn increment_packet_sequence<'e>(
    executor: impl Executor<'e, Database = Db>,
    chain_id: &ChainId,
) -> Result<Chain> {
    let raw: RawChain = sqlx::query_as(
        "UPDATE chains SET packet_sequence = packet_sequence + 1, updated_at = $1 WHERE id = $2 RETURNING *",
    )
    .bind(Utc::now())
    .bind(chain_id.to_string())
    .fetch_one(executor)
    .await
    .context("unable to increment packet sequence of a chain")?;

    raw.try_into()
}
