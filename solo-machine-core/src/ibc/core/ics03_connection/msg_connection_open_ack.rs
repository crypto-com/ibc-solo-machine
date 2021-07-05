use cosmos_sdk_proto::ibc::core::connection::v1::MsgConnectionOpenAck;

const TYPE_URL: &str = "/ibc.core.connection.v1.MsgConnectionOpenAck";

impl_any_conversion!(MsgConnectionOpenAck, TYPE_URL);
