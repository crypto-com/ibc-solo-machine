use ibc_proto::ibc::lightclients::solomachine::v3::Header;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v3.Header";

impl_any_conversion!(Header, TYPE_URL);
