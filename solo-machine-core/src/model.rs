//! Data types used by solo machine
pub(crate) mod chain;
pub(crate) mod ibc;
pub(crate) mod operation;

pub use self::{
    chain::{
        chain_keys::ChainKey,
        {Chain, ChainConfig, ConnectionDetails, Fee},
    },
    operation::{Operation, OperationType},
};
