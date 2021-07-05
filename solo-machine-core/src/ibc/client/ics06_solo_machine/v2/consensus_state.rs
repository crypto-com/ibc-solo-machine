use crate::proto::ibc::lightclients::solomachine::v2::ConsensusState;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v2.ConsensusState";

impl_any_conversion!(ConsensusState, TYPE_URL);
