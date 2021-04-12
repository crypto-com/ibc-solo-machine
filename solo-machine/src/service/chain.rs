tonic::include_proto!("chain");

use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use anyhow::{Context, Error, Result};
use ibc::core::ics24_host::identifier::ChainId;
use num_rational::{ParseRatioError, Ratio};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sled::Tree;
use tendermint::node::Id as NodeId;
use tendermint_rpc::{Client, HttpClient};
use time::OffsetDateTime;
use tonic::{Request, Response, Status};

use self::chain_server::Chain as IChain;

const DEFAULT_ACCOUNT_PREFIX: &str = "cosmos";
const DEFAULT_DIVERSIFIER: &str = "solo-machine-diversifier";
const DEFAULT_GRPC_ADDR: &str = "http://0.0.0.0:9090";
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
    ) -> Result<ChainId> {
        let tendermint_client = HttpClient::new(rpc_addr.as_str())?;
        let status = tendermint_client.status().await?;

        let chain_id: ChainId = status.node_info.network.to_string().parse()?;
        let node_id: NodeId = status.node_info.id;

        let consensus_timestamp = OffsetDateTime::now_utc()
            .unix_timestamp()
            .try_into()
            .context("unable to convert unix timestamp to u64")?;

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
        };

        Ok(Response::new(response))
    }
}
