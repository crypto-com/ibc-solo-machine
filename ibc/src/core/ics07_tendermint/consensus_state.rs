use std::time::SystemTime;

use cosmos_sdk_proto::ibc::{
    core::commitment::v1::MerkleRoot, lightclients::tendermint::v1::ConsensusState,
};
use prost_types::Timestamp;
use tendermint::block::Header;

pub trait IConsensusState {
    fn from_header(header: Header) -> Self;
}

impl IConsensusState for ConsensusState {
    fn from_header(header: Header) -> Self {
        Self {
            root: Some(MerkleRoot {
                hash: header.app_hash.value(),
            }),
            timestamp: Some(Timestamp::from(SystemTime::from(header.time))),
            next_validators_hash: header.next_validators_hash.as_bytes().to_vec(),
        }
    }
}
