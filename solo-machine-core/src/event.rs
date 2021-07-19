use anyhow::{anyhow, Result};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    cosmos::crypto::PublicKey,
    ibc::core::ics24_host::identifier::{ChainId, ChannelId, ClientId, ConnectionId, Identifier},
    model::ConnectionDetails,
};

/// Events emitted by IBC service
#[allow(clippy::large_enum_variant)]
pub enum Event {
    // ----- IBC events ----- //
    /// Sent tokens from solo machine to IBC enabled chain
    TokensSent {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Address of account on solo machine
        from_address: String,
        /// Address of account on IBC enabled chain
        to_address: String,
        /// Amount of tokens minted
        amount: u32,
        /// Denom of tokens minted
        denom: Identifier,
    },
    /// Received tokens to solo machine from IBC enabled chain
    TokensReceived {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Address of account on IBC enabled chain
        from_address: String,
        /// Address of account on IBC solo machine
        to_address: String,
        /// Amount of tokens minted
        amount: u32,
        /// Denom of tokens minted
        denom: Identifier,
    },
    /// Updated signer's public key on IBC enabled change for future messages from solo machine
    SignerUpdated {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Old signer's public key
        old_public_key: PublicKey,
        /// New signer's public key
        new_public_key: PublicKey,
    },

    // ----- IBC connection handshake events ----- //
    /// Created solo machine client on IBC enabled chain
    CreatedSoloMachineClient {
        /// Client ID of solo machine client on IBC enabled chain
        client_id: ClientId,
    },
    /// Created tendermint client on solo machine
    CreatedTendermintClient {
        /// Client ID of IBC enabled chain on solo machine
        client_id: ClientId,
    },
    /// Initialized connection on IBC enabled chain
    InitializedConnectionOnTendermint {
        /// Connection ID of solo machine client on IBC enabled chain
        connection_id: ConnectionId,
    },
    /// Initialized connection on solo machine
    InitializedConnectionOnSoloMachine {
        /// Connection ID of IBC enabled chain on solo machine
        connection_id: ConnectionId,
    },
    /// Confirmed connection on IBC enabled chain
    ConfirmedConnectionOnTendermint {
        /// Connection ID of solo machine client on IBC enabled chain
        connection_id: ConnectionId,
    },
    /// Confirmed connection on solo machine
    ConfirmedConnectionOnSoloMachine {
        /// Connection ID of IBC enabled chain on solo machine
        connection_id: ConnectionId,
    },
    /// Initialized channel on IBC enabled chain
    InitializedChannelOnTendermint {
        /// Channel ID of solo machine client on IBC enabled chain
        channel_id: ChannelId,
    },
    /// Initialized channel on solo machine
    InitializedChannelOnSoloMachine {
        /// Channel ID of IBC enabled chain on solo machine
        channel_id: ChannelId,
    },
    /// Confirmed channel on IBC enabled chain
    ConfirmedChannelOnTendermint {
        /// Channel ID of solo machine client on IBC enabled chain
        channel_id: ChannelId,
    },
    /// Confirmed channel on solo machine
    ConfirmedChannelOnSoloMachine {
        /// Channel ID of IBC enabled chain on solo machine
        channel_id: ChannelId,
    },
    /// Connection successfully established
    ConnectionEstablished {
        /// Chain ID of IBC enabled chain
        chain_id: ChainId,
        /// Connection details
        connection_details: ConnectionDetails,
    },

    // ----- Chain events ----- //
    /// Added new chain metadata to solo machine
    ChainAdded {
        /// Chain ID
        chain_id: ChainId,
    },

    // ----- Bank events ----- //
    /// Minted new tokens on solo machine
    TokensMinted {
        /// Address of account
        address: String,
        /// Amount of tokens minted
        amount: u32,
        /// Denom of tokens minted
        denom: Identifier,
    },
    /// Burnt tokens on solo machine
    TokensBurnt {
        /// Address of account
        address: String,
        /// Amount of tokens burnt
        amount: u32,
        /// Denom of tokens burnt
        denom: Identifier,
    },
}

pub(crate) fn notify_event(notifier: &Option<UnboundedSender<Event>>, event: Event) -> Result<()> {
    match notifier {
        None => Ok(()),
        Some(ref notifier) => notifier
            .send(event)
            .map_err(|err| anyhow!("unable to send event to notifier: {}", err)),
    }
}
