use anyhow::Result;
use structopt::StructOpt;

use self::command::Command;

mod command;
mod event;
mod output;
mod server;
mod signer;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv::dotenv();
    Command::from_args().execute().await
}
