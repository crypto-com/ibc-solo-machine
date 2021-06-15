use cosmos_sdk_proto::ibc::applications::transfer::v1::MsgTransfer;

const TYPE_URL: &str = "/ibc.applications.transfer.v1.MsgTransfer";

impl_any_conversion!(MsgTransfer, TYPE_URL);
