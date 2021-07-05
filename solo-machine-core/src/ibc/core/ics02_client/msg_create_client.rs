use cosmos_sdk_proto::ibc::core::client::v1::MsgCreateClient;

const TYPE_URL: &str = "/ibc.core.client.v1.MsgCreateClient";

impl_any_conversion!(MsgCreateClient, TYPE_URL);
