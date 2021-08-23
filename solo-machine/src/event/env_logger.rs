use anyhow::Result;
use async_trait::async_trait;
use solo_machine_core::{event::EventHandler, Event};

pub struct EnvLogger {}

impl EnvLogger {
    pub fn new() -> Self {
        env_logger::init();

        Self {}
    }
}

#[async_trait]
impl EventHandler for EnvLogger {
    async fn handle(&self, event: Event) -> Result<()> {
        match event {
            Event::TokensMinted {
                chain_id,
                request_id,
                to_address,
                amount,
                denom,
                transaction_hash,
            } => log::info!(
                "Minted new tokens [Chain ID = {}] [Request ID = {}] [Address = {}] [Amount = {} {}] [Transaction Hash = {}]",
                chain_id,
                request_id.unwrap_or_else(|| "None".to_string()),
                to_address,
                amount,
                denom,
                transaction_hash,
            ),
            Event::TokensBurnt {
                chain_id,
                request_id,
                from_address,
                amount,
                denom,
                transaction_hash,
            } => log::info!(
                "Burnt tokens [Chain ID = {}] [Request ID = {}] [Address = {}] [Amount = {} {}] [Transaction Hash = {}]",
                chain_id,
                request_id.unwrap_or_else(|| "None".to_string()),
                from_address,
                amount,
                denom,
                transaction_hash,
            ),
            Event::SignerUpdated {
                chain_id,
                old_public_key: _,
                new_public_key: _,
            } => log::info!(
                "Successfully updated signer's public key [Chain ID: {}]",
                chain_id
            ),

            Event::CreatedSoloMachineClient { client_id } => {
                log::info!(
                    "Created solo machine client on IBC enabled chain [Client ID = {}]",
                    client_id
                )
            }
            Event::CreatedTendermintClient { client_id } => log::info!(
                "Created tendermint client on solo machine [Client ID = {}]",
                client_id
            ),
            Event::InitializedConnectionOnTendermint { connection_id } => log::info!(
                "Initialized connection on IBC enabled chain [Connection ID = {}]",
                connection_id
            ),
            Event::InitializedConnectionOnSoloMachine { connection_id } => log::info!(
                "Initialized connection on solo machine [Connection ID = {}]",
                connection_id
            ),
            Event::ConfirmedConnectionOnTendermint { connection_id } => log::info!(
                "Confirmed connection on IBC enabled chain [Connection ID = {}]",
                connection_id
            ),
            Event::ConfirmedConnectionOnSoloMachine { connection_id } => log::info!(
                "Confirmed connection on solo machine [Connection ID = {}]",
                connection_id
            ),
            Event::InitializedChannelOnTendermint { channel_id } => log::info!(
                "Initialized channel on IBC enabled chain [Channel ID = {}]",
                channel_id
            ),
            Event::InitializedChannelOnSoloMachine { channel_id } => log::info!(
                "Initialized channel on solo machine [Channel ID = {}]",
                channel_id
            ),
            Event::ConfirmedChannelOnTendermint { channel_id } => log::info!(
                "Confirmed channel on IBC enabled chain [Channel ID = {}]",
                channel_id
            ),
            Event::ConfirmedChannelOnSoloMachine { channel_id } => log::info!(
                "Confirmed channel on solo machine [Channel ID = {}]",
                channel_id
            ),
            Event::ConnectionEstablished {
                chain_id,
                connection_details,
            } => log::info!(
                "Connection successfully established [Chain ID = {}] [Details = {}]",
                chain_id,
                serde_json::to_string(&connection_details)?
            ),
            Event::ChainAdded { chain_id } => {
                log::info!("Added new chain [Chain ID = {}]", chain_id)
            }
            Event::Warning { message } => log::warn!("{}", message),
        }

        Ok(())
    }
}
