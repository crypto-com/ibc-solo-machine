use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use structopt::StructOpt;
use tokio::runtime::Runtime;

use crate::server::Server;

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Starts the gRPC server
    Start {
        /// gRPC server address
        #[structopt(short, long, env = "SOLO_ADDR", default_value = "127.0.0.1:9000")]
        addr: SocketAddr,
        /// Path to storage directory
        #[structopt(short, long, env = "SOLO_STORAGE", default_value = ".solo-machine")]
        storage: PathBuf,
    },
}

impl Command {
    pub fn run(&self) -> Result<()> {
        match self {
            Self::Start {
                ref addr,
                ref storage,
            } => {
                let server = Server::new(storage)?;
                let runtime = Runtime::new()?;
                runtime.block_on(async { server.start(*addr).await })
            }
        }
    }
}
