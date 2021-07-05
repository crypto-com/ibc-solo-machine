use crate::proto::ibc::lightclients::solomachine::v2::Header;

const TYPE_URL: &str = "/ibc.lightclients.solomachine.v2.Header";

impl_any_conversion!(Header, TYPE_URL);
