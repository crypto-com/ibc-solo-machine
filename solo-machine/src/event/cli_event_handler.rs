use std::{fmt::Display, io::Write};

use anyhow::{Context, Result};
use async_trait::async_trait;
use cli_table::{
    format::Justify, print_stdout, Cell, Color, ColorChoice, Row, RowStruct, Style, Table,
};
use serde_json::json;
use solo_machine_core::{event::EventHandler, Event};
use termcolor::{ColorSpec, StandardStream};

use crate::{
    command::{print_json, print_stream},
    output::OutputType,
};

pub struct CliEventHandler {
    color_choice: ColorChoice,
    output: OutputType,
}

impl CliEventHandler {
    pub fn new(color_choice: ColorChoice, output: OutputType) -> Self {
        Self {
            color_choice,
            output,
        }
    }

    fn handle_text_output(&self, event: Event) -> Result<()> {
        let mut stdout = StandardStream::stdout(self.color_choice);

        match event {
            Event::ChainAdded { chain_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    "New chain added!",
                )?;

                writeln!(stdout)?;

                let table = vec![vec![
                    "Chain ID".cell().bold(true),
                    format!("{}", chain_id)
                        .cell()
                        .bold(true)
                        .foreground_color(Some(Color::Green))
                        .justify(Justify::Right),
                ]]
                .table()
                .color_choice(self.color_choice);

                print_stdout(table).context("unable to print table to stdout")?;
            }
            Event::CloseChannelInitOnSoloMachine {
                chain_id,
                channel_id,
            } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Chain channel init closed! [Chain ID = {}] [Channel ID = {}]",
                        chain_id, channel_id
                    ),
                )?;
                writeln!(stdout)?;
            }
            Event::TokensMinted {
                chain_id,
                request_id,
                to_address,
                amount,
                denom,
                transaction_hash,
            } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    "Tokens minted!",
                )?;
                writeln!(stdout)?;

                let mut table = Vec::new();

                add_row(&mut table, "Chain ID", chain_id);
                add_row(
                    &mut table,
                    "Request ID",
                    request_id.as_deref().unwrap_or("-"),
                );
                add_row(&mut table, "To", to_address);
                add_row(&mut table, "Amount", amount);
                add_row(&mut table, "Denom", denom);
                add_row(&mut table, "Transaction Hash", transaction_hash);

                print_stdout(table.table().color_choice(self.color_choice))
                    .context("unable to print table to stdout")?;
            }
            Event::TokensBurnt {
                chain_id,
                request_id,
                from_address,
                amount,
                denom,
                transaction_hash,
            } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    "Tokens burnt!",
                )?;
                writeln!(stdout)?;

                let mut table = Vec::new();

                add_row(&mut table, "Chain ID", chain_id);
                add_row(
                    &mut table,
                    "Request ID",
                    request_id.as_deref().unwrap_or("-"),
                );
                add_row(&mut table, "From", from_address);
                add_row(&mut table, "Amount", amount);
                add_row(&mut table, "Denom", denom);
                add_row(&mut table, "Transaction Hash", transaction_hash);

                print_stdout(table.table().color_choice(self.color_choice))
                    .context("unable to print table to stdout")?;
            }
            Event::SignerUpdated { chain_id, .. } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    "Signer updated!",
                )?;
                writeln!(stdout)?;

                let mut table = Vec::new();

                add_row(&mut table, "Chain ID", chain_id);

                print_stdout(table.table().color_choice(self.color_choice))
                    .context("unable to print table to stdout")?;
            }
            Event::CreatedSoloMachineClient { client_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Created solo machine client on IBC enabled chain [Client ID = {}]",
                        client_id
                    ),
                )?;
            }
            Event::CreatedTendermintClient { client_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Created tendermint client on solo machine [Client ID = {}]",
                        client_id
                    ),
                )?;
            }
            Event::InitializedConnectionOnTendermint { connection_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Initialized connection on IBC enabled chain [Connection ID = {}]",
                        connection_id
                    ),
                )?;
            }
            Event::InitializedConnectionOnSoloMachine { connection_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Initialized connection on solo machine [Connection ID = {}]",
                        connection_id
                    ),
                )?;
            }
            Event::ConfirmedConnectionOnTendermint { connection_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Confirmed connection on IBC enabled chain [Connection ID = {}]",
                        connection_id
                    ),
                )?;
            }
            Event::ConfirmedConnectionOnSoloMachine { connection_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Confirmed connection on solo machine [Connection ID = {}]",
                        connection_id
                    ),
                )?;
            }
            Event::InitializedChannelOnTendermint { channel_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Initialized channel on IBC enabled chain [Channel ID = {}]",
                        channel_id
                    ),
                )?;
            }
            Event::InitializedChannelOnSoloMachine { channel_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Initialized channel on solo machine [Channel ID = {}]",
                        channel_id
                    ),
                )?;
            }
            Event::ConfirmedChannelOnTendermint { channel_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Confirmed channel on IBC enabled chain [Channel ID = {}]",
                        channel_id
                    ),
                )?;
            }
            Event::ConfirmedChannelOnSoloMachine { channel_id } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    format!(
                        "Confirmed channel on solo machine [Channel ID = {}]",
                        channel_id
                    ),
                )?;
            }
            Event::ConnectionEstablished {
                chain_id,
                connection_details,
            } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true),
                    "Connection established!",
                )?;
                writeln!(stdout)?;

                let mut table = Vec::new();

                add_row(&mut table, "Chain ID", chain_id);
                add_row(
                    &mut table,
                    "Solo machine client ID",
                    connection_details.solo_machine_client_id,
                );
                add_row(
                    &mut table,
                    "Tendermint client ID",
                    connection_details.tendermint_client_id,
                );
                add_row(
                    &mut table,
                    "Solo machine connection ID",
                    connection_details.solo_machine_connection_id,
                );
                add_row(
                    &mut table,
                    "Tendermint connection ID",
                    connection_details.tendermint_connection_id,
                );
                add_row(
                    &mut table,
                    "Solo machine channel ID",
                    connection_details.solo_machine_channel_id,
                );
                add_row(
                    &mut table,
                    "Tendermint channel ID",
                    connection_details.tendermint_channel_id,
                );

                print_stdout(table.table().color_choice(self.color_choice))
                    .context("unable to print table to stdout")?;
            }
            Event::Warning { message } => {
                print_stream(
                    &mut stdout,
                    ColorSpec::new().set_bold(true).set_fg(Some(Color::Yellow)),
                    format!("WARNING: {}", message),
                )?;
            }
        }

        Ok(())
    }

    fn handle_json_output(&self, event: Event) -> Result<()> {
        match event {
            Event::Warning { message } => print_json(
                self.color_choice,
                json!({
                    "result": "warning",
                    "data": message,
                }),
            ),
            _ => print_json(
                self.color_choice,
                json!({
                    "result": "success",
                    "data": event,
                }),
            ),
        }
    }
}

#[async_trait]
impl EventHandler for CliEventHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        match self.output {
            OutputType::Text => self.handle_text_output(event),
            OutputType::Json => self.handle_json_output(event),
        }
    }
}

fn add_row(table: &mut Vec<RowStruct>, title: &str, value: impl Display) {
    table.push(vec![title.cell().bold(true), value.cell()].row());
}
