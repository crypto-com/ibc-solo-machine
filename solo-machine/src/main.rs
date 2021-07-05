use anyhow::Result;
use command::Command;
use structopt::StructOpt;

mod command;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv::dotenv();
    Command::from_args().execute().await
}
