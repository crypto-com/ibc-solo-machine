use anyhow::{Context, Result};
use k256::ecdsa::VerifyingKey;
use solo_machine_core::{
    cosmos::crypto::{PublicKey, PublicKeyAlgo},
    ibc::core::ics24_host::identifier::{ChainId, Identifier},
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
    },
    /// Sends some tokens to IBC enabled chain
    Send {
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
    },
    /// Receives some tokens from IBC enabled chain
    Receive {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Amount to receive from IBC enabled chain
        amount: u32,
        /// Denom of tokens to receive from IBC enabled chain
        denom: Identifier,
        /// Optional receiver address (if this is not provided, tokens will be received to signer's address)
        receiver: Option<String>,
        /// Optional memo to include in transactions
        #[structopt(
            long,
            default_value = "solo-machine-memo",
            env = "SOLO_MEMO",
            hide_env_values = true
        )]
        memo: String,
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
}

impl IbcCommand {
    pub async fn execute(
        self,
        db_pool: DbPool,
        signer: impl Signer,
        sender: UnboundedSender<Event>,
    ) -> Result<()> {
        let ibc_service = IbcService::new_with_notifier(db_pool, sender);

        match self {
            Self::Connect { chain_id, memo } => ibc_service.connect(signer, chain_id, memo).await,
            Self::Send {
                chain_id,
                amount,
                denom,
                receiver,
                memo,
            } => {
                ibc_service
                    .send_to_chain(signer, chain_id, amount, denom, receiver, memo)
                    .await
            }
            Self::Receive {
                chain_id,
                amount,
                denom,
                receiver,
                memo,
            } => {
                ibc_service
                    .receive_from_chain(signer, chain_id, amount, denom, receiver, memo)
                    .await
            }
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
        }
    }
}
