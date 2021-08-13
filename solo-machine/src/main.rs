use anyhow::Result;
use command::Command;
use structopt::StructOpt;

mod command;
mod event;
mod server;
mod signer;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv::dotenv();
    Command::from_args().execute().await
}
