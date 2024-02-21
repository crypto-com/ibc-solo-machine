use ibc_proto::ibc::lightclients::solomachine::v3::ConsensusState;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v3.ConsensusState";

impl_any_conversion!(ConsensusState, TYPE_URL);
