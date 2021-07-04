mod command;
mod crypto;
mod handler;
mod server;
mod service;
mod transaction_builder;

use anyhow::Result;
use structopt::StructOpt;

use self::command::Command;

fn main() -> Result<()> {
    env_logger::init();

    let command = Command::from_args();
    command.run()
}
