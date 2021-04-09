mod bank_service;
mod command;
mod crypto;
mod server;
mod service;

use anyhow::Result;
use structopt::StructOpt;

use self::command::Command;

fn main() -> Result<()> {
    env_logger::init();

    let command = Command::from_args();
    command.run()
}
