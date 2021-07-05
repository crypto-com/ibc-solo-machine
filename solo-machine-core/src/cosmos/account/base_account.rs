use cosmos_sdk_proto::cosmos::auth::v1beta1::BaseAccount;

pub const TYPE_URL: &str = "/cosmos.auth.v1beta1.BaseAccount";

impl_any_conversion!(BaseAccount, TYPE_URL);
