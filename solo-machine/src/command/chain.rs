use std::{io::Write, time::Duration};

use anyhow::{ensure, Context, Result};
use cli_table::{format::Justify, print_stdout, Cell, Row, RowStruct, Style, Table};
use humantime::format_duration;
use num_rational::Ratio;
use rust_decimal::Decimal;
use serde_json::json;
use solo_machine_core::{
    ibc::core::ics24_host::identifier::{ChainId, Identifier, PortId},
    model::{ChainConfig, ChainKey, Fee},
    service::ChainService,
    DbPool, Event, ToPublicKey,
};
use structopt::StructOpt;
use tendermint::block::Height as BlockHeight;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tokio::sync::mpsc::UnboundedSender;

use crate::{command::add_row, output::OutputType};

use super::print_json;

#[derive(Debug, StructOpt)]
pub enum ChainCommand {
    /// Adds metadata for new IBC enabled chain
    Add {
        /// gRPC address of IBC enabled chain
        #[structopt(
            long,
            default_value = "http://0.0.0.0:9090",
            env = "SOLO_GRPC_ADDRESS",
            hide_env_values = true
        )]
        grpc_addr: String,
        /// RPC address of IBC enabled chain
        #[structopt(
            long,
            default_value = "http://0.0.0.0:26657",
            env = "SOLO_RPC_ADDRESS",
            hide_env_values = true
        )]
        rpc_addr: String,
        /// Fee amount
        #[structopt(
            long,
            default_value = "1000",
            env = "SOLO_FEE_AMOUNT",
            hide_env_values = true
        )]
        fee_amount: Decimal,
        /// Fee denom
        #[structopt(
            long,
            default_value = "stake",
            env = "SOLO_FEE_DENOM",
            hide_env_values = true
        )]
        fee_denom: Identifier,
        /// Gas limit
        #[structopt(
            long,
            default_value = "300000",
            env = "SOLO_GAS_LIMIT",
            hide_env_values = true
        )]
        gas_limit: u64,
        /// Trust level (e.g. 1/3)
        #[structopt(
            long,
            default_value = "1/3",
            env = "SOLO_TRUST_LEVEL",
            hide_env_values = true
        )]
        trust_level: Ratio<u64>,
        /// Trusting period
        #[structopt(
            long,
            default_value = "14 days",
            env = "SOLO_TRUSTING_PERIOD",
            hide_env_values = true,
            parse(try_from_str = humantime::parse_duration)
        )]
        trusting_period: Duration,
        /// Maximum clock drift
        #[structopt(
            long,
            default_value = "3 sec",
            env = "SOLO_MAX_CLOCK_DRIFT",
            hide_env_values = true,
            parse(try_from_str = humantime::parse_duration)
        )]
        max_clock_drift: Duration,
        /// RPC timeout duration
        #[structopt(
            long,
            default_value = "60 sec",
            env = "SOLO_RPC_TIMEOUT",
            hide_env_values = true,
            parse(try_from_str = humantime::parse_duration)
        )]
        rpc_timeout: Duration,
        /// Diversifier used in transactions for chain
        #[structopt(
            long,
            default_value = "solo-machine-diversifier",
            env = "SOLO_DIVERSIFIER",
            hide_env_values = true
        )]
        diversifier: String,
        /// Port ID used to create connection with chain
        #[structopt(
            long,
            default_value = "transfer",
            env = "SOLO_PORT_ID",
            hide_env_values = true
        )]
        port_id: PortId,
        /// Trusted height of the chain
        #[structopt(long, env = "SOLO_TRUSTED_HEIGHT", hide_env_values = true)]
        trusted_height: BlockHeight,
        /// Block hash at trusted height of the chain
        #[structopt(long, env = "SOLO_TRUSTED_HASH", hide_env_values = true, parse(try_from_str = parse_trusted_hash))]
        trusted_hash: [u8; 32],
    },
    /// Fetches current state and metadata for an IBC enabled chain
    Get { chain_id: ChainId },
    /// Fetches all the public keys associated with solo machine client on given chain
    GetPublicKeys {
        chain_id: ChainId,
        #[structopt(long, default_value = "10")]
        limit: u32,
        #[structopt(long, default_value)]
        offset: u32,
    },
    /// Returns the final denom of a token on solo machine after sending it on given chain
    GetIbcDenom {
        chain_id: ChainId,
        denom: Identifier,
    },
    /// Fetches balance of given denom on IBC enabled chain
    Balance {
        chain_id: ChainId,
        denom: Identifier,
    },
}

impl ChainCommand {
    pub async fn execute(
        self,
        db_pool: DbPool,
        signer: impl ToPublicKey,
        sender: UnboundedSender<Event>,
        color_choice: ColorChoice,
        output: OutputType,
    ) -> Result<()> {
        let chain_service = ChainService::new_with_notifier(db_pool, sender);

        match self {
            Self::Add {
                grpc_addr,
                rpc_addr,
                fee_amount,
                fee_denom,
                gas_limit,
                trust_level,
                trusting_period,
                max_clock_drift,
                rpc_timeout,
                diversifier,
                port_id,
                trusted_height,
                trusted_hash,
            } => {
                let config = ChainConfig {
                    grpc_addr,
                    rpc_addr,
                    fee: Fee {
                        amount: fee_amount,
                        denom: fee_denom,
                        gas_limit,
                    },
                    trust_level,
                    trusting_period,
                    max_clock_drift,
                    rpc_timeout,
                    diversifier,
                    port_id,
                    trusted_height,
                    trusted_hash,
                };

                chain_service
                    .add(&config, &signer.to_public_key()?.encode())
                    .await
                    .map(|_| ())
            }
            Self::Get { ref chain_id } => {
                let chain = chain_service.get(chain_id).await?;

                match chain {
                    None => match output {
                        OutputType::Text => {
                            let mut stdout = StandardStream::stdout(color_choice);
                            stdout.set_color(
                                ColorSpec::new().set_bold(true).set_fg(Some(Color::Red)),
                            )?;
                            writeln!(&mut stdout, "Chain with id `{}` not found!", chain_id)
                                .context("unable to write to stdout")?;
                            stdout.reset().context("unable to reset stdout")
                        }
                        OutputType::Json => print_json(
                            color_choice,
                            json!({
                                "result": "error",
                                "data": format!("Chain with id `{}` not found!", chain_id)
                            }),
                        ),
                    },
                    Some(ref chain) => match output {
                        OutputType::Text => {
                            let mut table = Vec::new();

                            add_row(&mut table, "ID", &chain.id);
                            add_row(&mut table, "Node ID", chain.node_id);
                            add_row(&mut table, "gRPC address", &chain.config.grpc_addr);
                            add_row(&mut table, "RPC address", &chain.config.rpc_addr);
                            add_row(&mut table, "Fee amount", chain.config.fee.amount);
                            add_row(&mut table, "Fee denom", &chain.config.fee.denom);
                            add_row(&mut table, "Gas limit", chain.config.fee.gas_limit);
                            add_row(&mut table, "Trust level", chain.config.trust_level);
                            add_row(
                                &mut table,
                                "Trusting period",
                                format_duration(chain.config.trusting_period),
                            );
                            add_row(
                                &mut table,
                                "Maximum clock drift",
                                format_duration(chain.config.max_clock_drift),
                            );
                            add_row(
                                &mut table,
                                "RPC timeout",
                                format_duration(chain.config.rpc_timeout),
                            );
                            add_row(&mut table, "Diversifier", &chain.config.diversifier);
                            add_row(&mut table, "Port ID", &chain.config.port_id);
                            add_row(&mut table, "Trusted height", chain.config.trusted_height);
                            add_row(
                                &mut table,
                                "Trusted hash",
                                hex::encode_upper(chain.config.trusted_hash),
                            );
                            add_row(&mut table, "Consensus timestamp", chain.consensus_timestamp);
                            add_row(&mut table, "Sequence", chain.sequence);
                            add_row(&mut table, "Packet sequence", chain.packet_sequence);

                            match chain.connection_details {
                                None => table.push(
                                    vec![
                                        "Connection status".cell().bold(true),
                                        "Not Connected".cell().foreground_color(Some(Color::Red)),
                                    ]
                                    .row(),
                                ),
                                Some(ref connection_details) => {
                                    table.push(
                                        vec![
                                            "Connection status".cell().bold(true),
                                            "Connected".cell().foreground_color(Some(Color::Green)),
                                        ]
                                        .row(),
                                    );

                                    add_row(
                                        &mut table,
                                        "Solo machine client ID",
                                        &connection_details.solo_machine_client_id,
                                    );
                                    add_row(
                                        &mut table,
                                        "Tendermint client ID",
                                        &connection_details.tendermint_client_id,
                                    );
                                    add_row(
                                        &mut table,
                                        "Solo machine connection ID",
                                        &connection_details.solo_machine_connection_id,
                                    );
                                    add_row(
                                        &mut table,
                                        "Tendermint connection ID",
                                        &connection_details.tendermint_connection_id,
                                    );
                                    add_row(
                                        &mut table,
                                        "Solo machine channel ID",
                                        connection_details
                                            .solo_machine_channel_id
                                            .as_ref()
                                            .unwrap(),
                                    );
                                    add_row(
                                        &mut table,
                                        "Tendermint channel ID",
                                        connection_details.tendermint_channel_id.as_ref().unwrap(),
                                    );
                                }
                            }

                            add_row(&mut table, "Created at", chain.created_at);
                            add_row(&mut table, "Updated at", chain.updated_at);

                            print_stdout(table.table().color_choice(color_choice))
                                .context("unable to print table to stdout")
                        }
                        OutputType::Json => print_json(
                            color_choice,
                            json!({
                                "result": "success",
                                "data": chain
                            }),
                        ),
                    },
                }
            }
            Self::GetPublicKeys {
                ref chain_id,
                limit,
                offset,
            } => {
                let keys = chain_service
                    .get_public_keys(chain_id, limit, offset)
                    .await?;

                match output {
                    OutputType::Text => {
                        let table = keys
                            .into_iter()
                            .map(into_row)
                            .collect::<Vec<RowStruct>>()
                            .table()
                            .title(vec![
                                "ID".cell().bold(true),
                                "Chain ID".cell().bold(true),
                                "Public key".cell().bold(true),
                                "Created at".cell().bold(true),
                            ])
                            .color_choice(color_choice);

                        print_stdout(table).context("unable to print table to stdout")
                    }
                    OutputType::Json => print_json(
                        color_choice,
                        json!({
                            "result": "success",
                            "data": keys
                        }),
                    ),
                }
            }
            Self::GetIbcDenom {
                ref chain_id,
                ref denom,
            } => {
                let ibc_denom = chain_service.get_ibc_denom(chain_id, denom).await?;

                match output {
                    OutputType::Text => {
                        let table = vec![vec![
                            "IBC denom".cell().bold(true),
                            ibc_denom
                                .cell()
                                .bold(true)
                                .foreground_color(Some(Color::Green))
                                .justify(Justify::Right),
                        ]]
                        .table()
                        .color_choice(color_choice);

                        print_stdout(table).context("unable to print table to stdout")
                    }
                    OutputType::Json => print_json(
                        color_choice,
                        json!({
                            "result": "success",
                            "data": ibc_denom
                        }),
                    ),
                }
            }
            Self::Balance { chain_id, denom } => {
                let balance = chain_service.balance(signer, &chain_id, &denom).await?;

                match output {
                    OutputType::Text => {
                        let table = vec![vec![
                            "Balance".cell().bold(true),
                            format!("{} {}", balance, denom).cell(),
                        ]]
                        .table()
                        .color_choice(color_choice);

                        print_stdout(table).context("unable to print table to stdout")
                    }
                    OutputType::Json => print_json(
                        color_choice,
                        json!({
                            "result": "success",
                            "data": {
                                "balance": balance,
                                "denom": denom,
                            }
                        }),
                    ),
                }
            }
        }
    }
}

fn parse_trusted_hash(hash: &str) -> Result<[u8; 32]> {
    ensure!(!hash.is_empty(), "empty trusted hash");

    let bytes = hex::decode(hash).context("invalid trusted hash hex bytes")?;
    ensure!(bytes.len() == 32, "trusted hash length should be 32");

    let mut trusted_hash = [0; 32];
    trusted_hash.clone_from_slice(&bytes);

    Ok(trusted_hash)
}

fn into_row(key: ChainKey) -> RowStruct {
    vec![
        key.id.cell().justify(Justify::Right),
        key.chain_id.cell(),
        key.public_key.cell(),
        key.created_at.cell(),
    ]
    .row()
}
