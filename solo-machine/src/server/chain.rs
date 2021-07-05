tonic::include_proto!("chain");

use std::{
    convert::{TryFrom, TryInto},
    num::TryFromIntError,
    time::{Duration, SystemTime},
};

use solo_machine_core::{
    model::{ChainConfig as CoreChainConfig, Fee},
    service::ChainService as CoreChainService,
    DbPool, Signer,
};
use tonic::{Request, Response, Status};

use self::chain_server::Chain;

const DEFAULT_GRPC_ADDR: &str = "http://0.0.0.0:9090";
const DEFAULT_RPC_ADDR: &str = "http://0.0.0.0:26657";
const DEFAULT_FEE_AMOUNT: &str = "1000";
const DEFAULT_FEE_DENOM: &str = "stake";
const DEFAULT_GAS_LIMIT: u64 = 300000;
const DEFAULT_TRUST_LEVEL: &str = "1/3";
const DEFAULT_TRUSTING_PERIOD: Duration = Duration::from_secs(336 * 60 * 60); // 14 days
const DEFAULT_MAX_CLOCK_DRIFT: Duration = Duration::from_secs(3); // 3 secs
const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(60); // 60 secs
const DEFAULT_DIVERSIFIER: &str = "solo-machine-diversifier";
const DEFAULT_PORT_ID: &str = "transfer";

pub struct ChainService<S> {
    core_service: CoreChainService,
    signer: S,
}

impl<S> ChainService<S> {
    /// Creates a new instance of gRPC chain service
    pub fn new(db_pool: DbPool, signer: S) -> Self {
        let core_service = CoreChainService::new(db_pool);

        Self {
            core_service,
            signer,
        }
    }
}

#[tonic::async_trait]
impl<S> Chain for ChainService<S>
where
    S: Signer + Send + Sync + 'static,
{
    async fn add(
        &self,
        request: Request<AddChainRequest>,
    ) -> Result<Response<AddChainResponse>, Status> {
        let request = request.into_inner();

        let config = request
            .config
            .ok_or_else(|| Status::invalid_argument("config must be provided"))?;

        let grpc_addr = config
            .grpc_addr
            .unwrap_or_else(|| DEFAULT_GRPC_ADDR.to_string());
        let rpc_addr = config
            .rpc_addr
            .unwrap_or_else(|| DEFAULT_RPC_ADDR.to_string());

        let fee_config = config.fee_config.unwrap_or_else(|| FeeConfig {
            fee_amount: Some(DEFAULT_FEE_AMOUNT.to_string()),
            fee_denom: Some(DEFAULT_FEE_DENOM.to_string()),
            gas_limit: Some(DEFAULT_GAS_LIMIT),
        });

        let fee = Fee {
            amount: fee_config
                .fee_amount
                .unwrap_or_else(|| DEFAULT_FEE_AMOUNT.to_string())
                .parse()
                .map_err(|err: rust_decimal::Error| Status::invalid_argument(err.to_string()))?,
            denom: fee_config
                .fee_denom
                .unwrap_or_else(|| DEFAULT_FEE_DENOM.to_string())
                .parse()
                .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?,
            gas_limit: fee_config.gas_limit.unwrap_or(DEFAULT_GAS_LIMIT),
        };

        let trust_level = config
            .trust_level
            .unwrap_or_else(|| DEFAULT_TRUST_LEVEL.to_string())
            .parse()
            .map_err(|e: num_rational::ParseRatioError| Status::invalid_argument(e.to_string()))?;

        let trusting_period = config
            .trusting_period
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative trusting_period"))?
            .unwrap_or(DEFAULT_TRUSTING_PERIOD);

        let max_clock_drift = config
            .max_clock_drift
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative max_clock_drift"))?
            .unwrap_or(DEFAULT_MAX_CLOCK_DRIFT);

        let rpc_timeout = config
            .rpc_timeout
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| Status::invalid_argument("negative rpc_timeout"))?
            .unwrap_or(DEFAULT_RPC_TIMEOUT);

        let diversifier = config
            .diversifier
            .unwrap_or_else(|| DEFAULT_DIVERSIFIER.to_string());

        let port_id = config
            .port_id
            .unwrap_or_else(|| DEFAULT_PORT_ID.to_string())
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let trusted_height = config
            .trusted_height
            .ok_or_else(|| Status::invalid_argument("trusted_height must be provided"))?
            .into();

        let trusted_hash_str = config
            .trusted_hash
            .ok_or_else(|| Status::invalid_argument("trusted_hash must be provided"))?;

        if trusted_hash_str.is_empty() {
            return Err(Status::invalid_argument("trusted_hash is empty"));
        }

        let trusted_hash_bytes = hex::decode(&trusted_hash_str)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        if trusted_hash_bytes.len() != 32 {
            return Err(Status::invalid_argument("trusted_hash length should be 32"));
        }

        let mut trusted_hash = [0; 32];
        trusted_hash.copy_from_slice(&trusted_hash_bytes);

        let core_config = CoreChainConfig {
            grpc_addr,
            rpc_addr,
            fee,
            trust_level,
            trusting_period,
            max_clock_drift,
            rpc_timeout,
            diversifier,
            port_id,
            trusted_height,
            trusted_hash,
        };

        let chain_id = self
            .core_service
            .add(&core_config)
            .await
            .map_err(|err| Status::internal(err.to_string()))?
            .to_string();

        Ok(Response::new(AddChainResponse { chain_id }))
    }

    async fn query(
        &self,
        request: Request<QueryChainRequest>,
    ) -> Result<Response<QueryChainResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let chain = self
            .core_service
            .get(&chain_id)
            .await
            .map_err(|err| Status::internal(err.to_string()))?
            .ok_or_else(|| Status::not_found("chain details not found"))?;

        let response = QueryChainResponse {
            chain_id: chain.id.to_string(),
            node_id: chain.node_id.to_string(),
            config: Some(ChainConfig {
                grpc_addr: Some(chain.config.grpc_addr),
                rpc_addr: Some(chain.config.rpc_addr),
                fee_config: Some(FeeConfig {
                    fee_amount: Some(chain.config.fee.amount.to_string()),
                    fee_denom: Some(chain.config.fee.denom.to_string()),
                    gas_limit: Some(chain.config.fee.gas_limit),
                }),
                trust_level: Some(chain.config.trust_level.to_string()),
                trusting_period: Some(chain.config.trusting_period.into()),
                max_clock_drift: Some(chain.config.max_clock_drift.into()),
                rpc_timeout: Some(chain.config.rpc_timeout.into()),
                diversifier: Some(chain.config.diversifier),
                port_id: Some(chain.config.port_id.to_string()),
                trusted_height: Some(
                    u64::from(chain.config.trusted_height)
                        .try_into()
                        .map_err(|err: TryFromIntError| Status::internal(err.to_string()))?,
                ),
                trusted_hash: Some(hex::encode(chain.config.trusted_hash)),
            }),
            consensus_timestamp: Some(SystemTime::from(chain.consensus_timestamp).into()),
            sequence: chain.sequence,
            packet_sequence: chain.packet_sequence,
            connection_details: chain.connection_details.map(|connection_details| {
                ConnectionDetails {
                    solo_machine_client_id: connection_details.solo_machine_client_id.to_string(),
                    tendermint_client_id: connection_details.tendermint_client_id.to_string(),
                    solo_machine_connection_id: connection_details
                        .solo_machine_connection_id
                        .to_string(),
                    tendermint_connection_id: connection_details
                        .tendermint_connection_id
                        .to_string(),
                    solo_machine_channel_id: connection_details.solo_machine_channel_id.to_string(),
                    tendermint_channel_id: connection_details.tendermint_channel_id.to_string(),
                }
            }),
            created_at: Some(SystemTime::from(chain.created_at).into()),
            updated_at: Some(SystemTime::from(chain.updated_at).into()),
        };

        Ok(Response::new(response))
    }

    async fn get_ibc_denom(
        &self,
        request: Request<GetIbcDenomRequest>,
    ) -> Result<Response<GetIbcDenomResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let ibc_denom = self
            .core_service
            .get_ibc_denom(&chain_id, &denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        let response = GetIbcDenomResponse { ibc_denom };

        Ok(Response::new(response))
    }

    async fn query_balance(
        &self,
        request: Request<QueryBalanceRequest>,
    ) -> Result<Response<QueryBalanceResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let balance = self
            .core_service
            .balance(&self.signer, &chain_id, &denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?
            .to_string();

        let response = QueryBalanceResponse { balance };

        Ok(Response::new(response))
    }
}
