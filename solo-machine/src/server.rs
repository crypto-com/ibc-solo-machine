mod chain;
mod ibc;

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use solo_machine_core::{DbPool, Event, Signer};
use tokio::sync::mpsc::UnboundedSender;
use tonic::transport::Server as GrpcServer;

use self::{
    chain::{chain_server::ChainServer, ChainService},
    ibc::{ibc_server::IbcServer, IbcService},
};

/// Starts gRPC server
pub async fn start_grpc(
    db_pool: DbPool,
    signer: impl Signer + Clone + 'static,
    sender: UnboundedSender<Event>,
    addr: SocketAddr,
) -> Result<()> {
    let chain_service = ChainService::new(db_pool.clone(), sender.clone(), signer.clone());
    let ibc_service = IbcService::new(db_pool, sender, signer);

    log::info!("starting grpc server at {}", addr);

    GrpcServer::builder()
        .timeout(Duration::from_secs(60))
        .add_service(ChainServer::new(chain_service))
        .add_service(IbcServer::new(ibc_service))
        .serve(addr)
        .await
        .context(format!("unable to start grpc server at: {}", addr))
}
