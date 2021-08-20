use anyhow::{Context, Result};
use cli_table::{
    format::Justify, print_stdout, Cell, Color, ColorChoice, Row, RowStruct, Style, Table,
};
use k256::ecdsa::VerifyingKey;
use solo_machine_core::{
    cosmos::crypto::{PublicKey, PublicKeyAlgo},
    ibc::core::ics24_host::identifier::{ChainId, Identifier},
    model::{Operation, OperationType},
    service::IbcService,
    DbPool, Event, Signer,
};
use structopt::StructOpt;
use tokio::sync::mpsc::UnboundedSender;

const PUBLIC_KEY_ALGO_VARIANTS: [&str; 2] = ["secp256k1", "eth-secp256k1"];

#[derive(Debug, StructOpt)]
pub enum IbcCommand {
    /// Establishes connection with an IBC enabled chain
    Connect {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Optional memo to include in transactions
        #[structopt(
            long,
            default_value = "solo-machine-memo",
            env = "SOLO_MEMO",
            hide_env_values = true
        )]
        memo: String,
        /// Force create a new connection even if one already exists
        #[structopt(long)]
        force: bool,
    },
    /// Mint some tokens on IBC enabled chain
    Mint {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Amount to send to IBC enabled chain
        amount: u32,
        /// Denom of tokens to send to IBC enabled chain
        denom: Identifier,
        /// Optional receiver address (if this is not provided, tokens will be sent to signer's address)
        receiver: Option<String>,
        /// Optional memo to include in transactions
        #[structopt(
            long,
            default_value = "solo-machine-memo",
            env = "SOLO_MEMO",
            hide_env_values = true
        )]
        memo: String,
        /// Optional request ID (for tracking purposes)
        #[structopt(long)]
        request_id: Option<String>,
    },
    /// Burn some tokens on IBC enabled chain
    Burn {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Amount to receive from IBC enabled chain
        amount: u32,
        /// Denom of tokens to receive from IBC enabled chain
        denom: Identifier,
        /// Optional memo to include in transactions
        #[structopt(
            long,
            default_value = "solo-machine-memo",
            env = "SOLO_MEMO",
            hide_env_values = true
        )]
        memo: String,
        /// Optional request ID (for tracking purposes)
        #[structopt(long)]
        request_id: Option<String>,
    },
    /// Updates signer's public key on IBC enabled chain for future messages from solo machine
    UpdateSigner {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Hex encoded public key
        #[structopt(long, env = "SOLO_NEW_PUBLIC_KEY", hide_env_values = true)]
        new_public_key: String,
        /// Type of public key
        #[structopt(long, possible_values = &PUBLIC_KEY_ALGO_VARIANTS, default_value = "secp256k1", env = "SOLO_PUBLIC_KEY_ALGO", hide_env_values = true)]
        public_key_algo: PublicKeyAlgo,
        /// Optional memo to include in transactions
        #[structopt(
            long,
            default_value = "solo-machine-memo",
            env = "SOLO_MEMO",
            hide_env_values = true
        )]
        memo: String,
    },
    /// Check history of operations on solo machine
    History {
        #[structopt(long, default_value = "10")]
        limit: u32,
        #[structopt(long, default_value)]
        offset: u32,
    },
}

impl IbcCommand {
    pub async fn execute(
        self,
        db_pool: DbPool,
        signer: impl Signer,
        sender: UnboundedSender<Event>,
        color_choice: ColorChoice,
    ) -> Result<()> {
        let ibc_service = IbcService::new_with_notifier(db_pool, sender);

        match self {
            Self::Connect {
                chain_id,
                memo,
                force,
            } => ibc_service.connect(signer, chain_id, memo, force).await,
            Self::Mint {
                chain_id,
                amount,
                denom,
                receiver,
                memo,
                request_id,
            } => ibc_service
                .mint(signer, chain_id, request_id, amount, denom, receiver, memo)
                .await
                .map(|_| ()),
            Self::Burn {
                chain_id,
                amount,
                denom,
                memo,
                request_id,
            } => ibc_service
                .burn(signer, chain_id, request_id, amount, denom, memo)
                .await
                .map(|_| ()),
            Self::UpdateSigner {
                chain_id,
                new_public_key,
                public_key_algo,
                memo,
            } => {
                let new_public_key_bytes =
                    hex::decode(&new_public_key).context("unable to decode hex bytes")?;

                let new_verifying_key = VerifyingKey::from_sec1_bytes(&new_public_key_bytes)
                    .context("invalid secp256k1 bytes")?;

                let new_public_key = match public_key_algo {
                    PublicKeyAlgo::Secp256k1 => PublicKey::Secp256k1(new_verifying_key),
                    #[cfg(feature = "ethermint")]
                    PublicKeyAlgo::EthSecp256k1 => PublicKey::EthSecp256k1(new_verifying_key),
                };

                ibc_service
                    .update_signer(signer, chain_id, new_public_key, memo)
                    .await
            }
            Self::History { limit, offset } => {
                let history = ibc_service.history(signer, limit, offset).await?;

                let table = history
                    .into_iter()
                    .map(into_row)
                    .collect::<Vec<RowStruct>>()
                    .table()
                    .title(vec![
                        "ID".cell().bold(true),
                        "Request ID".cell().bold(true),
                        "Address".cell().bold(true),
                        "Denom".cell().bold(true),
                        "Amount".cell().bold(true),
                        "Type".cell().bold(true),
                        "Transaction Hash".cell().bold(true),
                        "Time".cell().bold(true),
                    ])
                    .color_choice(color_choice);

                print_stdout(table).context("unable to print table to stdout")
            }
        }
    }
}

fn into_row(operation: Operation) -> RowStruct {
    let color = get_color_for_operation_type(&operation.operation_type);

    vec![
        operation.id.cell().justify(Justify::Right),
        operation
            .request_id
            .unwrap_or_else(|| "-".to_string())
            .cell(),
        operation.address.cell(),
        operation.denom.cell(),
        operation.amount.cell().justify(Justify::Right),
        operation
            .operation_type
            .cell()
            .foreground_color(Some(color)),
        operation.transaction_hash.cell(),
        operation.created_at.cell(),
    ]
    .row()
}

fn get_color_for_operation_type(operation_type: &OperationType) -> Color {
    match operation_type {
        OperationType::Mint { .. } => Color::Green,
        OperationType::Burn { .. } => Color::Red,
    }
}
