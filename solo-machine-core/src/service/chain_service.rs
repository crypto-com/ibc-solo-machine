use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use tendermint::node::Id as NodeId;
use tendermint_rpc::{Client, HttpClient};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    event::notify_event,
    ibc::core::ics24_host::identifier::{ChainId, Identifier},
    model::{chain, Chain, ChainConfig},
    DbPool, Event, ToPublicKey,
};

/// Used to manage IBC enabled chain's state and metadata
pub struct ChainService {
    db_pool: DbPool,
    notifier: Option<UnboundedSender<Event>>,
}

impl ChainService {
    /// Creates a new instance of chain service
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            notifier: None,
        }
    }

    /// Creates a new instance of chain service with notifier
    pub fn new_with_notifier(db_pool: DbPool, notifier: UnboundedSender<Event>) -> Self {
        Self {
            db_pool,
            notifier: Some(notifier),
        }
    }

    /// Add details of an IBC enabled chain
    pub async fn add(&self, config: &ChainConfig) -> Result<ChainId> {
        let tendermint_client = HttpClient::new(config.rpc_addr.as_str())?;
        let status = tendermint_client.status().await?;

        let chain_id: ChainId = status.node_info.network.to_string().parse()?;
        let node_id: NodeId = status.node_info.id;

        chain::add_chain(&self.db_pool, &chain_id, &node_id, config).await?;

        notify_event(
            &self.notifier,
            Event::ChainAdded {
                chain_id: chain_id.clone(),
            },
        )?;

        Ok(chain_id)
    }

    /// Returns the final denom of a token on solo machine after sending it on given chain
    pub async fn get_ibc_denom(&self, chain_id: &ChainId, denom: &Identifier) -> Result<String> {
        let chain = self
            .get(chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details not found when computing ibc denom"))?;
        chain
            .get_ibc_denom(denom)
            .ok_or_else(|| anyhow!("connection details not found when computing ibc denom"))
    }

    /// Fetches details of a chain
    pub async fn get(&self, chain_id: &ChainId) -> Result<Option<Chain>> {
        chain::get_chain(&self.db_pool, chain_id).await
    }

    /// Fetches balance of given denom on IBC enabled chain
    pub async fn balance(
        &self,
        signer: impl ToPublicKey,
        chain_id: &ChainId,
        denom: &Identifier,
    ) -> Result<Decimal> {
        let chain = self
            .get(chain_id)
            .await?
            .ok_or_else(|| anyhow!("chain details not found when fetching balance"))?;

        chain.get_balance(signer, denom).await
    }
}
