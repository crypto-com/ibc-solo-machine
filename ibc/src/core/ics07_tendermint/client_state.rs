use std::cmp::Ordering;

use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::lightclients::tendermint::v1::{ClientState, ConsensusState, Header};

use crate::core::ics02_client::height::IHeight;

use super::{consensus_state::IConsensusState as _, header::IHeader as _};

pub trait IClientState: Sized {
    type Header;
    type ConsensusState;

    fn check_header_and_update_state(
        self,
        header: Self::Header,
    ) -> Result<(Self, Self::ConsensusState)>;
}

impl IClientState for ClientState {
    type Header = Header;
    type ConsensusState = ConsensusState;

    fn check_header_and_update_state(
        mut self,
        header: Header,
    ) -> Result<(Self, Self::ConsensusState)> {
        let header_height = header.get_height()?;

        if Ordering::Greater
            == header_height.cmp(
                self.latest_height
                    .as_ref()
                    .ok_or_else(|| anyhow!("client state does not have latest height"))?,
            )
        {
            self.latest_height = Some(header_height);
        }

        let consensus_state = ConsensusState::from_header(header)?;

        // TODO: set client state's processed time

        Ok((self, consensus_state))
    }
}

const TYPE_URL: &str = "/ibc.lightclients.tendermint.v1.ClientState";

impl_any_conversion!(ClientState, TYPE_URL);
