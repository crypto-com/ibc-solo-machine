tonic::include_proto!("ibc");

use k256::ecdsa::VerifyingKey;
use solo_machine_core::{
    cosmos::crypto::{PublicKey, PublicKeyAlgo},
    ibc::core::ics24_host::identifier::ChainId,
    service::IbcService as CoreIbcService,
    DbPool, Signer,
};
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

    async fn update_signer(
        &self,
        request: Request<UpdateSignerRequest>,
    ) -> Result<Response<UpdateSignerResponse>, Status> {
        let request = request.into_inner();

        let chain_id: ChainId = request
            .chain_id
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

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
            .update_signer(&self.signer, chain_id, new_public_key, memo)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(UpdateSignerResponse {}))
    }
}
