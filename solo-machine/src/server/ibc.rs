tonic::include_proto!("ibc");

use std::time::SystemTime;

use k256::ecdsa::VerifyingKey;
use primitive_types::U256;
use solo_machine_core::{
    cosmos::crypto::{PublicKey, PublicKeyAlgo},
    ibc::core::ics24_host::identifier::ChainId,
    service::IbcService as CoreIbcService,
    DbPool, Event, Signer,
};
use tokio::sync::mpsc::UnboundedSender;
use tonic::{Request, Response, Status};

use self::ibc_server::Ibc;

const DEFAULT_MEMO: &str = "solo-machine-memo";

pub struct IbcService<S> {
    core_service: CoreIbcService,
    signer: S,
}

impl<S> IbcService<S> {
    /// Creates a new instance of gRPC IBC service
    pub fn new(db_pool: DbPool, notifier: UnboundedSender<Event>, signer: S) -> Self {
        let core_service = CoreIbcService::new_with_notifier(db_pool, notifier);

        Self {
            core_service,
            signer,
        }
    }
}

#[tonic::async_trait]
impl<S> Ibc for IbcService<S>
where
    S: Signer + Send + Sync + 'static,
{
    async fn connect(
        &self,
        request: Request<ConnectRequest>,
    ) -> Result<Response<ConnectResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());
        let request_id = request.request_id;
        let force = request.force;

        self.core_service
            .connect(&self.signer, chain_id, request_id, memo, force)
            .await
            .map_err(|err| {
                log::error!("{}", err);
                Status::internal(err.to_string())
            })?;

        Ok(Response::new(ConnectResponse {}))
    }

    async fn mint(&self, request: Request<MintRequest>) -> Result<Response<MintResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let request_id = request.request_id;
        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());
        let amount = U256::from_dec_str(&request.amount)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let receiver = request.receiver_address;

        let transaction_hash = self
            .core_service
            .mint(
                &self.signer,
                chain_id,
                request_id,
                amount,
                denom,
                receiver,
                memo,
            )
            .await
            .map_err(|err| {
                log::error!("{}", err);
                Status::internal(err.to_string())
            })?;

        Ok(Response::new(MintResponse { transaction_hash }))
    }

    async fn burn(&self, request: Request<BurnRequest>) -> Result<Response<BurnResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let request_id = request.request_id;
        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());
        let amount = U256::from_dec_str(&request.amount)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let transaction_hash = self
            .core_service
            .burn(&self.signer, chain_id, request_id, amount, denom, memo)
            .await
            .map_err(|err| {
                log::error!("{}", err);
                Status::internal(err.to_string())
            })?;

        Ok(Response::new(BurnResponse { transaction_hash }))
    }

    async fn update_signer(
        &self,
        request: Request<UpdateSignerRequest>,
    ) -> Result<Response<UpdateSignerResponse>, Status> {
        let request = request.into_inner();

        let chain_id: ChainId = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let request_id = request.request_id;

        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());

        let new_public_key_bytes = hex::decode(&request.new_public_key)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        let new_verifying_key = VerifyingKey::from_sec1_bytes(&new_public_key_bytes)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        let public_key_algo = request
            .public_key_algo
            .map(|s| s.parse())
            .transpose()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?
            .unwrap_or(PublicKeyAlgo::Secp256k1);

        let new_public_key = match public_key_algo {
            PublicKeyAlgo::Secp256k1 => PublicKey::Secp256k1(new_verifying_key),
            #[cfg(feature = "ethermint")]
            PublicKeyAlgo::EthSecp256k1 => PublicKey::EthSecp256k1(new_verifying_key),
        };

        self.core_service
            .update_signer(&self.signer, chain_id, request_id, new_public_key, memo)
            .await
            .map_err(|err| {
                log::error!("{}", err);
                Status::internal(err.to_string())
            })?;

        Ok(Response::new(UpdateSignerResponse {}))
    }

    async fn query_history(
        &self,
        request: Request<QueryHistoryRequest>,
    ) -> Result<Response<QueryHistoryResponse>, Status> {
        let request = request.into_inner();

        let limit = i32::try_from(request.limit.unwrap_or(10))
            .or(Err(Status::invalid_argument("invalid `limit`")))?;
        let offset = i32::try_from(request.offset.unwrap_or(0))
            .or(Err(Status::invalid_argument("invalid `offset`")))?;

        let history = self
            .core_service
            .history(&self.signer, limit, offset)
            .await
            .map_err(|err| {
                log::error!("{}", err);
                Status::internal(err.to_string())
            })?;

        let response = QueryHistoryResponse {
            operations: history
                .into_iter()
                .map(|op| Operation {
                    id: op.id,
                    request_id: op.request_id,
                    address: op.address,
                    denom: op.denom.to_string(),
                    amount: op.amount.to_string(),
                    operation_type: op.operation_type.to_string(),
                    transaction_hash: op.transaction_hash,
                    created_at: Some(SystemTime::from(op.created_at).into()),
                })
                .collect(),
        };

        Ok(Response::new(response))
    }
}
