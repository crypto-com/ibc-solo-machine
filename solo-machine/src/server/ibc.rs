tonic::include_proto!("ibc");

use solo_machine_core::{service::IbcService as CoreIbcService, DbPool, Signer};
use tonic::{Request, Response, Status};

use self::ibc_server::Ibc;

const DEFAULT_MEMO: &str = "solo-machine-memo";

pub struct IbcService<S> {
    core_service: CoreIbcService,
    signer: S,
}

impl<S> IbcService<S> {
    /// Creates a new instance of gRPC IBC service
    pub fn new(db_pool: DbPool, signer: S) -> Self {
        let core_service = CoreIbcService::new(db_pool);

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

        self.core_service
            .connect(&self.signer, chain_id, memo)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(ConnectResponse {}))
    }

    async fn send_to_chain(
        &self,
        request: Request<SendToChainRequest>,
    ) -> Result<Response<SendToChainResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());
        let amount = request.amount;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let receiver = request.receiver_address;

        self.core_service
            .send_to_chain(&self.signer, chain_id, amount, denom, receiver, memo)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(SendToChainResponse {}))
    }

    async fn receive_from_chain(
        &self,
        request: Request<ReceiveFromChainRequest>,
    ) -> Result<Response<ReceiveFromChainResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let memo = request.memo.unwrap_or_else(|| DEFAULT_MEMO.to_owned());
        let amount = request.amount;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;
        let receiver = request.receiver_address;

        self.core_service
            .receive_from_chain(&self.signer, chain_id, amount, denom, receiver, memo)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(ReceiveFromChainResponse {}))
    }
}
