use std::{net::SocketAddr, path::Path, time::Duration};

use anyhow::{Context, Result};
use sled::{Db, Tree};
use tonic::transport::Server as GrpcServer;

use crate::{
    handler::{msg_handler::MsgHandler, query_handler::QueryHandler},
    service::{
        bank::{bank_server::BankServer, BankService},
        chain::{chain_server::ChainServer, ChainService},
        ibc::{ibc_server::IbcServer, IbcService},
    },
};

const BALANCES_TREE: &str = "balances";
const CHAINS_TREE: &str = "chains";
const IBC_TREE: &str = "ibc";

pub struct Server {
    db: Db,
}

impl Server {
    /// Creates a new server
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let db = sled::open(&path).context(format!(
            "unable to open database at: {}",
            path.as_ref().display()
        ))?;

        Ok(Self { db })
    }

    /// Starts grpc services
    pub async fn start(&self, addr: SocketAddr) -> Result<()> {
        let msg_handler = MsgHandler::new(self.ibc_tree()?);
        let query_handler = QueryHandler::new(self.ibc_tree()?);

        let bank_service = BankService::new(self.balances_tree()?);
        let chain_service = ChainService::new(self.chains_tree()?);
        let ibc_service = IbcService::new(msg_handler, query_handler, chain_service.clone());

        log::info!("starting grpc server at {}", addr);

        GrpcServer::builder()
            .timeout(Duration::from_secs(60))
            .add_service(BankServer::new(bank_service))
            .add_service(ChainServer::new(chain_service))
            .add_service(IbcServer::new(ibc_service))
            .serve(addr)
            .await
            .context(format!("unable to start grpc server at: {}", addr))
    }

    /// Returns balances tree of storage
    fn balances_tree(&self) -> Result<Tree> {
        self.db
            .open_tree(BALANCES_TREE)
            .context("unable to open balances storage tree")
    }

    /// Returns chains tree of storage
    fn chains_tree(&self) -> Result<Tree> {
        self.db
            .open_tree(CHAINS_TREE)
            .context("unable to open chains storage tree")
    }

    /// Returns ibc tree of storage
    fn ibc_tree(&self) -> Result<Tree> {
        self.db
            .open_tree(IBC_TREE)
            .context("unable to open ibc storage tree")
    }
}
