use cosmos_sdk_proto::ibc::core::channel::v1::MsgAcknowledgement;

const TYPE_URL: &str = "/ibc.core.channel.v1.MsgAcknowledgement";

impl_any_conversion!(MsgAcknowledgement, TYPE_URL);
