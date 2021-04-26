use cosmos_sdk_proto::ibc::core::channel::v1::MsgRecvPacket;

const TYPE_URL: &str = "/ibc.core.channel.v1.MsgRecvPacket";

impl_any_conversion!(MsgRecvPacket, TYPE_URL);
