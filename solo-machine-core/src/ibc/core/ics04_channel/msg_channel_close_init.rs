use cosmos_sdk_proto::ibc::core::channel::v1::MsgChannelCloseInit;

const TYPE_URL: &str = "/ibc.core.channel.v1.MsgChannelCloseInit";

impl_any_conversion!(MsgChannelCloseInit, TYPE_URL);
