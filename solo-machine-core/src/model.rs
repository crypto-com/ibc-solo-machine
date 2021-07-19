//! Data types used by solo machine
pub(crate) mod bank;
pub(crate) mod chain;
pub(crate) mod ibc;

pub use self::{
    bank::{
        account::Account,
        account_operation::{AccountOperation, OperationType},
    },
    chain::{
        chain_keys::ChainKey,
        {Chain, ChainConfig, ConnectionDetails, Fee},
    },
};
