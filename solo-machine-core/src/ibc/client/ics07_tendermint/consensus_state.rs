use ibc_proto::{
    google::protobuf::Timestamp,
    ibc::{core::commitment::v1::MerkleRoot, lightclients::tendermint::v1::ConsensusState},
};
use tendermint::block::Header as BlockHeader;

pub trait IConsensusState: Sized {
    fn from_block_header(header: BlockHeader) -> Self;
}

impl IConsensusState for ConsensusState {
    fn from_block_header(header: BlockHeader) -> Self {
        Self {
            root: Some(MerkleRoot {
                hash: header.app_hash.into(),
            }),
            timestamp: Some(Timestamp {
                seconds: header.time.unix_timestamp(),
                nanos: header.time.unix_timestamp_nanos() as i32,
            }),
            next_validators_hash: header.next_validators_hash.as_bytes().to_vec(),
        }
    }
}

const TYPE_URL: &str = "/ibc.lightclients.tendermint.v1.ConsensusState";

impl_any_conversion!(ConsensusState, TYPE_URL);
