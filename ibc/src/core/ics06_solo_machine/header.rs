use cosmos_sdk_proto::ibc::lightclients::solomachine::v1::Header;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v1.Header";

impl_any_conversion!(Header, TYPE_URL);
