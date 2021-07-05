use crate::proto::ibc::lightclients::solomachine::v2::ClientState;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v2.ClientState";

impl_any_conversion!(ClientState, TYPE_URL);
