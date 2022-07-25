use cosmos_sdk_proto::ibc::{
    core::commitment::v1::MerkleRoot, lightclients::tendermint::v1::ConsensusState,
};
use prost_types::Timestamp;
use tendermint::block::Header as BlockHeader;

pub trait IConsensusState: Sized {
    fn from_block_header(header: BlockHeader) -> Self;
}

impl IConsensusState for ConsensusState {
    fn from_block_header(header: BlockHeader) -> Self {
        let timestamp =
            cosmos_sdk_proto::tendermint::google::protobuf::Timestamp::from(header.time);

        Self {
            root: Some(MerkleRoot {
                hash: header.app_hash.value(),
            }),
            timestamp: Some(Timestamp {
                seconds: timestamp.seconds,
                nanos: timestamp.nanos,
            }),
            next_validators_hash: header.next_validators_hash.as_bytes().to_vec(),
        }
    }
}

const TYPE_URL: &str = "/ibc.lightclients.tendermint.v1.ConsensusState";

impl_any_conversion!(ConsensusState, TYPE_URL);
