tonic::include_proto!("chain");

use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use anyhow::{anyhow, Context, Error, Result};
use ibc::core::ics24_host::identifier::{ChainId, ChannelId, ClientId, ConnectionId, PortId};
use num_rational::{ParseRatioError, Ratio};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sled::{
    transaction::{ConflictableTransactionError, TransactionError},
    Tree,
};
use tendermint::{block::Height as BlockHeight, node::Id as NodeId};
use tendermint_rpc::{Client, HttpClient};
use time::OffsetDateTime;
use tonic::{Request, Response, Status};

use self::chain_server::Chain as IChain;

const DEFAULT_ACCOUNT_PREFIX: &str = "cosmos";
const DEFAULT_DIVERSIFIER: &str = "solo-machine-diversifier";
const DEFAULT_GRPC_ADDR: &str = "http://0.0.0.0:9090";
const DEFAULT_PORT_ID: &str = "transfer";
const DEFAULT_RPC_ADDR: &str = "http://0.0.0.0:26657";

#[derive(Debug, Serialize, Deserialize)]
pub struct Chain {
    pub id: ChainId,
    pub node_id: NodeId,
    pub grpc_addr: String,
    pub rpc_addr: String,
    pub account_prefix: String,
    pub fee: Fee,
    pub trust_level: Ratio<u64>,
    pub trusting_period: Duration,
    pub max_clock_drift: Duration,
    pub rpc_timeout: Duration,
    pub diversifier: String,
    pub consensus_timestamp: u64,
    pub sequence: u64,
    pub packet_sequence: u64,
    pub port_id: PortId,
    pub trusted_height: BlockHeight,
    pub trusted_hash: Option<[u8; 32]>,
    pub connection_details: Option<ChainConnectionDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConnectionDetails {
    pub solo_machine_client_id: ClientId,
    pub tendermint_client_id: ClientId,
    pub solo_machine_connection_id: ConnectionId,
    pub tendermint_connection_id: ConnectionId,
    pub solo_machine_channel_id: ChannelId,
    pub tendermint_channel_id: ChannelId,
}

impl From<&ChainConnectionDetails> for ConnectionDetails {
    fn from(value: &ChainConnectionDetails) -> Self {
        ConnectionDetails {
            solo_machine_client_id: value.solo_machine_client_id.to_string(),
            tendermint_client_id: value.tendermint_client_id.to_string(),
            solo_machine_connection_id: value.solo_machine_connection_id.to_string(),
            tendermint_connection_id: value.tendermint_connection_id.to_string(),
            solo_machine_channel_id: value.solo_machine_channel_id.to_string(),
            tendermint_channel_id: value.tendermint_channel_id.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fee {
    pub amount: Decimal,
    pub denom: String,
    pub gas_limit: u64,
}

impl Default for Fee {
    fn default() -> Self {
        Fee {
            amount: dec!(1000),
            denom: "stake".to_owned(),
            gas_limit: 300000,
        }
    }
}

#[derive(Clone)]
pub struct ChainService {
    tree: Tree,
}

impl ChainService {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn get(&self, chain_id: &ChainId) -> Result<Option<Chain>> {
        let bytes = self.tree.get(chain_id)?;

        match bytes {
            None => Ok(None),
            Some(bytes) => {
                let chain = serde_cbor::from_slice(&bytes)?;
                Ok(Some(chain))
            }
        }
    }

    pub fn increment_sequence(&self, chain_id: &ChainId) -> Result<Chain> {
        let response: Result<Chain, TransactionError<Error>> = self.tree.transaction(|tx| {
            let optional_bytes = tx.get(chain_id)?;

            match optional_bytes {
                None => Err(ConflictableTransactionError::Abort(anyhow!(
                    "chain with id {} not found",
                    chain_id
                ))),
                Some(bytes) => {
                    let mut chain: Chain = serde_cbor::from_slice(&bytes)
                        .context("unable to deserialize chain cbor bytes")
                        .map_err(ConflictableTransactionError::Abort)?;

                    chain.sequence += 1;

                    tx.insert(
                        chain_id.as_bytes(),
                        serde_cbor::to_vec(&chain)
                            .context("unable to serialize chain to cbor")
                            .map_err(ConflictableTransactionError::Abort)?,
                    )?;

                    Ok(chain)
                }
            }
        });

        match response {
            Ok(chain) => {
                log::info!(
                    "successfully incremented sequence for chain with id {}",
                    chain_id
                );
                Ok(chain)
            }
            Err(TransactionError::Storage(err)) => {
                Err(Error::from(err).context("storage error while executing transaction"))
            }
            Err(TransactionError::Abort(err)) => {
                Err(err.context("abort error while executing transaction"))
            }
        }
    }

    pub fn increment_packet_sequence(&self, chain_id: &ChainId) -> Result<Chain> {
        let response: Result<Chain, TransactionError<Error>> = self.tree.transaction(|tx| {
            let optional_bytes = tx.get(chain_id)?;

            match optional_bytes {
                None => Err(ConflictableTransactionError::Abort(anyhow!(
                    "chain with id {} not found",
                    chain_id
                ))),
                Some(bytes) => {
                    let mut chain: Chain = serde_cbor::from_slice(&bytes)
                        .context("unable to deserialize chain cbor bytes")
                        .map_err(ConflictableTransactionError::Abort)?;

                    chain.packet_sequence += 1;

                    tx.insert(
                        chain_id.as_bytes(),
                        serde_cbor::to_vec(&chain)
                            .context("unable to serialize chain to cbor")
                            .map_err(ConflictableTransactionError::Abort)?,
                    )?;

                    Ok(chain)
                }
            }
        });

        match response {
            Ok(chain) => {
                log::info!(
                    "successfully incremented packet sequence for chain with id {}",
                    chain_id
                );
                Ok(chain)
            }
            Err(TransactionError::Storage(err)) => {
                Err(Error::from(err).context("storage error while executing transaction"))
            }
            Err(TransactionError::Abort(err)) => {
                Err(err.context("abort error while executing transaction"))
            }
        }
    }

    pub fn add_connection_details(
        &self,
        chain_id: &ChainId,
        connection_details: &ChainConnectionDetails,
    ) -> Result<()> {
        let response: Result<(), TransactionError<Error>> = self.tree.transaction(|tx| {
            let optional_bytes = tx.get(chain_id)?;

            match optional_bytes {
                None => Err(ConflictableTransactionError::Abort(anyhow!(
                    "chain with id {} not found",
                    chain_id
                ))),
                Some(bytes) => {
                    let mut chain: Chain = serde_cbor::from_slice(&bytes)
                        .context("unable to deserialize chain cbor bytes")
                        .map_err(ConflictableTransactionError::Abort)?;

                    chain.connection_details = Some(connection_details.clone());

                    tx.insert(
                        chain_id.as_bytes(),
                        serde_cbor::to_vec(&chain)
                            .context("unable to serialize chain to cbor")
                            .map_err(ConflictableTransactionError::Abort)?,
                    )?;

                    Ok(())
                }
            }
        });

        match response {
            Ok(_) => {
                log::info!(
                    "successfully incremented sequence for chain with id {}",
                    chain_id
                );
                Ok(())
            }
            Err(TransactionError::Storage(err)) => {
                Err(Error::from(err).context("storage error while executing transaction"))
            }
            Err(TransactionError::Abort(err)) => {
                Err(err.context("abort error while executing transaction"))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn add(
        &self,
        grpc_addr: String,
        rpc_addr: String,
        account_prefix: String,
        fee: Fee,
        trust_level: Ratio<u64>,
        trusting_period: Duration,
        max_clock_drift: Duration,
        rpc_timeout: Duration,
        diversifier: String,
        port_id: PortId,
        trusted_height: BlockHeight,
        trusted_hash: Option<[u8; 32]>,
    ) -> Result<ChainId> {
        let tendermint_client = HttpClient::new(rpc_addr.as_str())?;
        let status = tendermint_client.status().await?;

        let chain_id: ChainId = status.node_info.network.to_string().parse()?;
        let node_id: NodeId = status.node_info.id;

        let consensus_timestamp = OffsetDateTime::now_utc()
            .unix_timestamp()
            .try_into()
            .context("unable to convert unix timestamp to u64")?;

        let sequence = 1;
        let packet_sequence = 1;

        let chain = Chain {
            id: chain_id.clone(),
            node_id,
            grpc_addr,
            rpc_addr,
            account_prefix,
            fee,
            trust_level,
            trusting_period,
            max_clock_drift,
            rpc_timeout,
            diversifier,
            consensus_timestamp,
            sequence,
            packet_sequence,
            port_id,
            trusted_height,
            trusted_hash,
            connection_details: None,
        };

        self.tree.insert(&chain_id, serde_cbor::to_vec(&chain)?)?;

        Ok(chain_id)
    }
}

#[tonic::async_trait]
impl IChain for ChainService {
    async fn add(
        &self,
        request: Request<AddChainRequest>,
    ) -> Result<Response<AddChainResponse>, Status> {
        let request = request.into_inner();

        let grpc_addr = request
            .grpc_addr
            .unwrap_or_else(|| DEFAULT_GRPC_ADDR.to_string());

        let rpc_addr = request
            .rpc_addr
            .unwrap_or_else(|| DEFAULT_RPC_ADDR.to_string());

        let account_prefix = request
            .account_prefix
            .unwrap_or_else(|| DEFAULT_ACCOUNT_PREFIX.to_string());

        let fee = match request.fee_config {
            None => Default::default(),
            Some(fee) => {
                let mut f = Fee::default();

                if let Some(amount) = fee.fee_amount {
                    f.amount = amount.parse().map_err(|e: rust_decimal::Error| {
                        Status::invalid_argument(e.to_string())
                    })?;
                }

                if let Some(denom) = fee.fee_denom {
                    f.denom = denom;
                }

                if let Some(gas_limit) = fee.gas_limit {
                    f.gas_limit = gas_limit.parse().map_err(|e: std::num::ParseIntError| {
                        Status::invalid_argument(e.to_string())
                    })?;
                }

                f
            }
        };

        let trust_level: Ratio<u64> = request
            .trust_level
            .unwrap_or_else(|| "1/3".to_string())
            .parse()
            .map_err(|e: ParseRatioError| Status::invalid_argument(e.to_string()))?;

        let trusting_period = request
            .trusting_period
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative trusting_period"))?
            .unwrap_or_else(|| Duration::from_secs(336 * 60 * 60));

        let max_clock_drift = request
            .max_clock_drift
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative max_clock_drift"))?
            .unwrap_or_else(|| Duration::from_millis(3000));

        let rpc_timeout = request
            .rpc_timeout
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative rpc_timeout"))?
            .unwrap_or_else(|| Duration::from_secs(60));

        let diversifier = request
            .diversifier
            .unwrap_or_else(|| DEFAULT_DIVERSIFIER.to_string());

        let port_id = request
            .port_id
            .unwrap_or_else(|| DEFAULT_PORT_ID.to_string())
            .parse()
            .map_err(|e| Status::invalid_argument(format!("invalid port id: {}", e)))?;

        let trusted_height = request
            .trusted_height
            .map(Into::into)
            .unwrap_or_else(|| 1u32.into());

        let trusted_hash: Option<[u8; 32]> = request
            .trusted_hash
            .and_then(|hash| {
                if hash.is_empty() {
                    return None;
                }

                let bytes: Vec<u8> = match hex::decode(&hash) {
                    Ok(bytes) => bytes,
                    Err(e) => return Some(Err(anyhow!("invalid trusted hash hex bytes: {}", e))),
                };

                if bytes.len() != 32 {
                    return Some(Err(anyhow!("trusted hash length should be 32")));
                }

                let mut trusted_hash = [0; 32];
                trusted_hash.clone_from_slice(&bytes);

                Some(Ok(trusted_hash))
            })
            .transpose()
            .map_err(|e| Status::invalid_argument(format!("invalid trusted hash: {}", e)))?;

        let chain_id = self
            .add(
                grpc_addr,
                rpc_addr,
                account_prefix,
                fee,
                trust_level,
                trusting_period,
                max_clock_drift,
                rpc_timeout,
                diversifier,
                port_id,
                trusted_height,
                trusted_hash,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = AddChainResponse {
            chain_id: chain_id.to_string(),
        };

        Ok(Response::new(response))
    }

    async fn query(
        &self,
        request: Request<QueryChainRequest>,
    ) -> Result<Response<QueryChainResponse>, Status> {
        let chain_id: ChainId = request
            .into_inner()
            .chain_id
            .parse()
            .map_err(|e: Error| Status::invalid_argument(e.to_string()))?;

        let chain = self
            .get(&chain_id)
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("details for {} not found", chain_id)))?;

        let response = QueryChainResponse {
            chain_id: chain_id.to_string(),
            node_id: chain.node_id.to_string(),
            grpc_addr: chain.grpc_addr,
            rpc_addr: chain.rpc_addr,
            account_prefix: chain.account_prefix,
            fee_config: Some(FeeConfig {
                fee_amount: Some(chain.fee.amount.to_string()),
                fee_denom: Some(chain.fee.denom),
                gas_limit: Some(chain.fee.gas_limit.to_string()),
            }),
            trust_level: chain.trust_level.to_string(),
            trusting_period: Some(chain.trusting_period.into()),
            max_clock_drift: Some(chain.max_clock_drift.into()),
            rpc_timeout: Some(chain.rpc_timeout.into()),
            diversifier: chain.diversifier.to_string(),
            sequence: chain.sequence,
            packet_sequence: chain.packet_sequence,
            port_id: chain.port_id.to_string(),
            trusted_height: chain.trusted_height.value(),
            trusted_hash: chain.trusted_hash.map(hex::encode).unwrap_or_default(),
            connection_details: chain.connection_details.as_ref().map(Into::into),
        };

        Ok(Response::new(response))
    }
}
