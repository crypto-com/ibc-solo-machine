//! Services exposed by solo machine
pub(crate) mod chain_service;
pub(crate) mod ibc_service;

pub use self::{chain_service::ChainService, ibc_service::IbcService};
