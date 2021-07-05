use cosmos_sdk_proto::ics23::{HashOp, InnerSpec, LeafOp, LengthOp, ProofSpec};

fn tendermint_spec() -> ProofSpec {
    ProofSpec {
        leaf_spec: Some(LeafOp {
            hash: HashOp::Sha256.into(),
            prehash_key: 0,
            prehash_value: HashOp::Sha256.into(),
            length: LengthOp::VarProto.into(),
            prefix: vec![0],
        }),
        inner_spec: Some(InnerSpec {
            child_order: vec![0, 1],
            min_prefix_length: 1,
            max_prefix_length: 1,
            child_size: 32,
            empty_child: vec![],
            hash: HashOp::Sha256.into(),
        }),
        max_depth: 0,
        min_depth: 0,
    }
}

fn iavl_spec() -> ProofSpec {
    ProofSpec {
        leaf_spec: Some(LeafOp {
            hash: HashOp::Sha256.into(),
            prehash_key: 0,
            prehash_value: HashOp::Sha256.into(),
            length: LengthOp::VarProto.into(),
            prefix: vec![0],
        }),
        inner_spec: Some(InnerSpec {
            child_order: vec![0, 1],
            min_prefix_length: 4,
            max_prefix_length: 12,
            child_size: 33,
            empty_child: vec![],
            hash: HashOp::Sha256.into(),
        }),
        max_depth: 0,
        min_depth: 0,
    }
}

pub fn proof_specs() -> Vec<ProofSpec> {
    vec![iavl_spec(), tendermint_spec()]
}
