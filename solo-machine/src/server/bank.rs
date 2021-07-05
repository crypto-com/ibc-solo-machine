tonic::include_proto!("bank");

use std::time::SystemTime;

use solo_machine_core::{service::BankService as CoreBankService, DbPool, Signer};
use tonic::{Request, Response, Status};

use self::bank_server::Bank;

pub struct BankService<S> {
    core_service: CoreBankService,
    signer: S,
}

impl<S> BankService<S> {
    /// Creates a new instance of gRPC bank service
    pub fn new(db_pool: DbPool, signer: S) -> Self {
        let core_service = CoreBankService::new(db_pool);

        Self {
            core_service,
            signer,
        }
    }
}

#[tonic::async_trait]
impl<S> Bank for BankService<S>
where
    S: Signer + Send + Sync + 'static,
{
    async fn mint(&self, request: Request<MintRequest>) -> Result<Response<MintResponse>, Status> {
        let request = request.into_inner();

        let amount = request.amount;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        self.core_service
            .mint(&self.signer, amount, denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(MintResponse {}))
    }

    async fn burn(&self, request: Request<BurnRequest>) -> Result<Response<BurnResponse>, Status> {
        let request = request.into_inner();

        let amount = request.amount;
        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        self.core_service
            .burn(&self.signer, amount, denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(BurnResponse {}))
    }

    async fn query_balance(
        &self,
        request: Request<QueryBalanceRequest>,
    ) -> Result<Response<QueryBalanceResponse>, Status> {
        let request = request.into_inner();

        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let balance = self
            .core_service
            .balance(&self.signer, &denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(QueryBalanceResponse {
            balance,
            denom: denom.to_string(),
        }))
    }

    async fn query_account(
        &self,
        request: Request<QueryAccountRequest>,
    ) -> Result<Response<QueryAccountResponse>, Status> {
        let request = request.into_inner();

        let denom = request
            .denom
            .parse()
            .map_err(|err: anyhow::Error| Status::invalid_argument(err.to_string()))?;

        let account = self
            .core_service
            .account(&self.signer, &denom)
            .await
            .map_err(|err| Status::internal(err.to_string()))?
            .ok_or_else(|| Status::not_found("account not found"))?;

        let response = QueryAccountResponse {
            address: account.address,
            denom: account.denom.to_string(),
            balance: account.balance,
            created_at: Some(SystemTime::from(account.created_at).into()),
            updated_at: Some(SystemTime::from(account.updated_at).into()),
        };

        Ok(Response::new(response))
    }

    async fn query_history(
        &self,
        request: Request<QueryHistoryRequest>,
    ) -> Result<Response<QueryHistoryResponse>, Status> {
        let request = request.into_inner();

        let limit = request.limit.unwrap_or(10);
        let offset = request.offset.unwrap_or(0);

        let history = self
            .core_service
            .history(&self.signer, limit, offset)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        let response = QueryHistoryResponse {
            operations: history
                .into_iter()
                .map(|op| AccountOperation {
                    id: op.id,
                    address: op.address,
                    denom: op.denom.to_string(),
                    amount: op.amount,
                    operation_type: op.operation_type.to_string(),
                    created_at: Some(SystemTime::from(op.created_at).into()),
                })
                .collect(),
        };

        Ok(Response::new(response))
    }
}
