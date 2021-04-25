use cosmos_sdk_proto::ibc::core::channel::v1::MsgChannelOpenAck;

const TYPE_URL: &str = "/ibc.core.channel.v1.MsgChannelOpenAck";

impl_any_conversion!(MsgChannelOpenAck, TYPE_URL);
