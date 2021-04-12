use std::convert::TryInto;

use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::{core::client::v1::Height, lightclients::tendermint::v1::Header};

use crate::core::ics24_host::identifier::ChainId;

pub trait IHeader {
    fn get_height(&self) -> Result<Height>;
}

impl IHeader for Header {
    fn get_height(&self) -> Result<Height> {
        let header = self
            .signed_header
            .as_ref()
            .ok_or_else(|| anyhow!("signed header not found in tendermint header"))?
            .header
            .as_ref()
            .ok_or_else(|| anyhow!("header not found in signed header"))?;

        let chain_id: ChainId = header.chain_id.parse()?;
        let height = header.height;

        Ok(Height {
            revision_number: chain_id.version(),
            revision_height: height.try_into()?,
        })
    }
}
