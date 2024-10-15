use anyhow::{ensure, Context, Result};
use ibc_proto::ibc::{
    core::{channel::v1::Channel, client::v1::Height, connection::v1::ConnectionEnd},
    lightclients::tendermint::v1::{
        ClientState as TendermintClientState, ConsensusState as TendermintConsensusState,
    },
};
use prost::Message;
use sqlx::{Executor, FromRow};

use crate::{
    ibc::core::ics24_host::{
        identifier::{ChannelId, ClientId, ConnectionId, PortId},
        path::{ChannelPath, ClientStatePath, ConnectionPath, ConsensusStatePath},
    },
    proto::proto_encode,
    Db,
};

#[derive(Debug, FromRow)]
struct IbcData {
    #[allow(dead_code)]
    path: String,
    data: Vec<u8>,
}

/// Adds tendermint client state to database
pub async fn add_tendermint_client_state<'e>(
    executor: impl Executor<'e, Database = Db>,
    client_id: &ClientId,
    client_state: &TendermintClientState,
) -> Result<()> {
    let path = ClientStatePath::new(client_id);
    let data = proto_encode(client_state)?;

    add(executor, path.get_key(0).unwrap(), &data).await
}

/// Fetches tendermint client state from database
pub async fn get_tendermint_client_state<'e>(
    executor: impl Executor<'e, Database = Db>,
    client_id: &ClientId,
) -> Result<Option<TendermintClientState>> {
    let path = ClientStatePath::new(client_id);
    get(executor, path.get_key(0).unwrap()).await
}

/// Adds tendermint consensus state to database
pub async fn add_tendermint_consensus_state<'e>(
    executor: impl Executor<'e, Database = Db>,
    client_id: &ClientId,
    height: &Height,
    consensus_state: &TendermintConsensusState,
) -> Result<()> {
    let path = ConsensusStatePath::new(client_id, height);
    let data = proto_encode(consensus_state)?;

    add(executor, path.get_key(0).unwrap(), &data).await
}

/// Adds connection to database
pub async fn add_connection<'e>(
    executor: impl Executor<'e, Database = Db>,
    connection_id: &ConnectionId,
    connection: &ConnectionEnd,
) -> Result<()> {
    let path = ConnectionPath::new(connection_id);
    let data = proto_encode(connection)?;

    add(executor, path.get_key(0).unwrap(), &data).await
}

/// Fetches connection from database
pub async fn get_connection<'e>(
    executor: impl Executor<'e, Database = Db>,
    connection_id: &ConnectionId,
) -> Result<Option<ConnectionEnd>> {
    let path = ConnectionPath::new(connection_id);
    get(executor, path.get_key(0).unwrap()).await
}

/// Updates connection in database
pub async fn update_connection<'e>(
    executor: impl Executor<'e, Database = Db>,
    connection_id: &ConnectionId,
    connection: &ConnectionEnd,
) -> Result<()> {
    let path = ConnectionPath::new(connection_id);
    let data = proto_encode(connection)?;

    update(executor, path.get_key(0).unwrap(), &data).await
}

/// Adds channel to database
pub async fn add_channel<'e>(
    executor: impl Executor<'e, Database = Db>,
    port_id: &PortId,
    channel_id: &ChannelId,
    channel: &Channel,
) -> Result<()> {
    let path = ChannelPath::new(port_id, channel_id);
    let data = proto_encode(channel)?;

    add(executor, path.get_key(0).unwrap(), &data).await
}

/// Fetches channel from database
pub async fn get_channel<'e>(
    executor: impl Executor<'e, Database = Db>,
    port_id: &PortId,
    channel_id: &ChannelId,
) -> Result<Option<Channel>> {
    let path = ChannelPath::new(port_id, channel_id);
    get(executor, path.get_key(0).unwrap()).await
}

/// Updates channel in database
pub async fn update_channel<'e>(
    executor: impl Executor<'e, Database = Db>,
    port_id: &PortId,
    channel_id: &ChannelId,
    channel: &Channel,
) -> Result<()> {
    let path = ChannelPath::new(port_id, channel_id);
    let data = proto_encode(channel)?;

    update(executor, path.get_key(0).unwrap(), &data).await
}

async fn add<'e>(
    executor: impl Executor<'e, Database = Db>,
    path: &str,
    data: &[u8],
) -> Result<()> {
    let rows_affected = sqlx::query("INSERT INTO ibc_data (path, data) VALUES ($1, $2)")
        .bind(path)
        .bind(data)
        .execute(executor)
        .await
        .context("unable to add ibc data in database")?
        .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when adding a new ibc data"
    );

    Ok(())
}

async fn update<'e>(
    executor: impl Executor<'e, Database = Db>,
    path: &str,
    data: &[u8],
) -> Result<()> {
    let rows_affected = sqlx::query("UPDATE ibc_data SET data = $1 where path = $2")
        .bind(data)
        .bind(path)
        .execute(executor)
        .await
        .context("unable to update ibc data in database")?
        .rows_affected();

    ensure!(
        rows_affected == 1,
        "rows_affected should be equal to 1 when updating ibc data"
    );

    Ok(())
}

async fn get<'e, M>(executor: impl Executor<'e, Database = Db>, path: &str) -> Result<Option<M>>
where
    M: Message + Default,
{
    sqlx::query_as("SELECT * FROM ibc_data WHERE path = $1")
        .bind(path)
        .fetch_optional(executor)
        .await?
        .map(|ibc_data: IbcData| M::decode(ibc_data.data.as_ref()))
        .transpose()
        .context("unable to decode protobuf bytes for ibc data")
}
