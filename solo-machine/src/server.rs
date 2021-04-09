use std::{net::SocketAddr, path::Path};

use anyhow::{Context, Result};
use sled::{Db, Tree};
use tonic::transport::Server as GrpcServer;

use crate::service::{
    bank::{bank_server::BankServer, BankService},
    ibc::{ibc_server::IbcServer, IbcService},
};

const BALANCES_TREE: &str = "balances";
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
        let bank_service = BankService::new(self.balances_tree()?);
        let ibc_service = IbcService::new(self.ibc_tree()?);

        log::info!("starting grpc server at {}", addr);

        GrpcServer::builder()
            .add_service(BankServer::new(bank_service))
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

    /// Returns ibc tree of storage
    fn ibc_tree(&self) -> Result<Tree> {
        self.db
            .open_tree(IBC_TREE)
            .context("unable to open ibc storage tree")
    }
}
