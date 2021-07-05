use cosmos_sdk_proto::ibc::core::channel::v1::MsgChannelOpenInit;

const TYPE_URL: &str = "/ibc.core.channel.v1.MsgChannelOpenInit";

impl_any_conversion!(MsgChannelOpenInit, TYPE_URL);
