use ibc_proto::ibc::core::connection::v1::MsgConnectionOpenInit;

const TYPE_URL: &str = "/ibc.core.connection.v1.MsgConnectionOpenInit";

impl_any_conversion!(MsgConnectionOpenInit, TYPE_URL);
