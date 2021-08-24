//! Different types of accounts in cosmos
#![allow(missing_docs)]

pub mod base_account;
#[cfg(feature = "ethermint")]
pub mod eth_account;

use anyhow::{anyhow, Result};
use cosmos_sdk_proto::cosmos::auth::v1beta1::BaseAccount;
use prost::Message;
use prost_types::Any;

#[cfg(feature = "ethermint")]
use crate::proto::ethermint::types::v1::EthAccount;
use crate::proto::AnyConvert;

use self::base_account::TYPE_URL as BASE_ACCOUNT_TYPE_URL;
#[cfg(feature = "ethermint")]
use self::eth_account::TYPE_URL as ETH_ACCOUNT_TYPE_URL;

#[derive(Debug)]
pub enum Account {
    Base(BaseAccount),
    #[cfg(feature = "ethermint")]
    Eth(EthAccount),
}

impl Account {
    pub fn get_base_account(&self) -> Option<&BaseAccount> {
        match self {
            Self::Base(ref account) => Some(account),
            #[cfg(feature = "ethermint")]
            Self::Eth(ref account) => account.get_base_account(),
        }
    }
}

impl AnyConvert for Account {
    fn from_any(value: &Any) -> Result<Self> {
        match value.type_url.as_str() {
            BASE_ACCOUNT_TYPE_URL => {
                let base_account: BaseAccount = BaseAccount::decode(value.value.as_slice())?;
                Ok(Self::Base(base_account))
            }
            #[cfg(feature = "ethermint")]
            ETH_ACCOUNT_TYPE_URL => {
                let eth_account: EthAccount = EthAccount::decode(value.value.as_slice())?;
                Ok(Self::Eth(eth_account))
            }
            other => Err(anyhow!("unknown type url for `Any` type: `{}`", other)),
        }
    }

    fn to_any(&self) -> Result<Any> {
        match self {
            Self::Base(ref account) => account.to_any(),
            #[cfg(feature = "ethermint")]
            Self::Eth(ref account) => account.to_any(),
        }
    }
}
