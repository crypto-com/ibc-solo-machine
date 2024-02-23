use ibc_proto::ibc::lightclients::solomachine::v3::ClientState;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v3.ClientState";

impl_any_conversion!(ClientState, TYPE_URL);
