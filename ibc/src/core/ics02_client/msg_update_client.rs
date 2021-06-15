use cosmos_sdk_proto::ibc::core::client::v1::MsgUpdateClient;

const TYPE_URL: &str = "/ibc.core.client.v1.MsgUpdateClient";

impl_any_conversion!(MsgUpdateClient, TYPE_URL);
