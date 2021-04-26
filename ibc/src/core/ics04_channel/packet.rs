use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::core::channel::v1::Packet;
use sha2::{Digest, Sha256};

pub trait IPacket {
    fn commitment_bytes(&self) -> Result<Vec<u8>>;
}

impl IPacket for Packet {
    fn commitment_bytes(&self) -> Result<Vec<u8>> {
        let timeout_height = self
            .timeout_height
            .as_ref()
            .ok_or_else(|| anyhow!("timeout height is not set"))?;

        let mut buf = Vec::new();

        buf.extend(&self.timeout_timestamp.to_be_bytes());
        buf.extend(&timeout_height.revision_number.to_be_bytes());
        buf.extend(&timeout_height.revision_height.to_be_bytes());
        buf.extend(Sha256::digest(&self.data));

        Ok(Sha256::digest(&buf).to_vec())
    }
}
