mod bank;
mod chain;
mod ibc;

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use solo_machine_core::{DbPool, Signer};
use tonic::transport::Server as GrpcServer;

use self::{
    bank::{bank_server::BankServer, BankService},
    chain::{chain_server::ChainServer, ChainService},
    ibc::{ibc_server::IbcServer, IbcService},
};

/// Starts gRPC server
pub async fn start_grpc(
    db_pool: DbPool,
    signer: impl Signer + Clone + Send + Sync + 'static,
    addr: SocketAddr,
) -> Result<()> {
    let bank_service = BankService::new(db_pool.clone(), signer.clone());
    let chain_service = ChainService::new(db_pool.clone(), signer.clone());
    let ibc_service = IbcService::new(db_pool, signer);

    GrpcServer::builder()
        .timeout(Duration::from_secs(60))
        .add_service(BankServer::new(bank_service))
        .add_service(ChainServer::new(chain_service))
        .add_service(IbcServer::new(ibc_service))
        .serve(addr)
        .await
        .context(format!("unable to start grpc server at: {}", addr))
}
