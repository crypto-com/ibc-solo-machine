use cosmos_sdk_proto::ibc::lightclients::solomachine::v1::ConsensusState;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v1.ConsensusState";

impl_any_conversion!(ConsensusState, TYPE_URL);
