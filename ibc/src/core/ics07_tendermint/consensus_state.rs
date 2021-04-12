use std::time::SystemTime;

use anyhow::{anyhow, Result};
use cosmos_sdk_proto::ibc::{
    core::commitment::v1::MerkleRoot,
    lightclients::tendermint::v1::{ConsensusState, Header},
};
use prost_types::Timestamp;
use tendermint::block::Header as BlockHeader;

pub trait IConsensusState: Sized {
    fn from_header(header: Header) -> Result<Self>;

    fn from_block_header(header: BlockHeader) -> Self;
}

impl IConsensusState for ConsensusState {
    fn from_header(header: Header) -> Result<Self> {
        let header = header
            .signed_header
            .ok_or_else(|| anyhow!("signed header not found in tendermint header"))?
            .header
            .ok_or_else(|| anyhow!("header not found in signed header"))?;

        Ok(Self {
            root: Some(MerkleRoot {
                hash: header.app_hash,
            }),
            timestamp: header.time.map(|t| Timestamp {
                seconds: t.seconds,
                nanos: t.nanos,
            }),
            next_validators_hash: header.next_validators_hash,
        })
    }

    fn from_block_header(header: BlockHeader) -> Self {
        Self {
            root: Some(MerkleRoot {
                hash: header.app_hash.value(),
            }),
            timestamp: Some(Timestamp::from(SystemTime::from(header.time))),
            next_validators_hash: header.next_validators_hash.as_bytes().to_vec(),
        }
    }
}
