use cosmos_sdk_proto::ibc::lightclients::tendermint::v1::ClientState;

const TYPE_URL: &str = "/ibc.lightclients.tendermint.v1.ClientState";

impl_any_conversion!(ClientState, TYPE_URL);
