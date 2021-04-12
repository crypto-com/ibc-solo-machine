use anyhow::{anyhow, ensure, Context, Result};
use bip39::Mnemonic;
use cosmos_sdk_proto::{
    cosmos::{
        auth::v1beta1::{
            query_client::QueryClient as AuthQueryClient, BaseAccount, QueryAccountRequest,
        },
        base::v1beta1::Coin,
        staking::v1beta1::{query_client::QueryClient as StakingQueryClient, QueryParamsRequest},
        tx::{
            signing::v1beta1::{
                signature_descriptor::{
                    data::{Single as SingleSignatureData, Sum as SignatureDataInner},
                    Data as SignatureData,
                },
                SignMode,
            },
            v1beta1::{
                mode_info::{Single, Sum},
                AuthInfo, Fee, ModeInfo, SignDoc, SignerInfo, TxBody, TxRaw,
            },
        },
    },
    ibc::{
        core::{
            client::v1::{Height, MsgCreateClient},
            commitment::v1::MerklePrefix,
            connection::v1::{
                Counterparty as ConnectionCounterparty, MsgConnectionOpenAck,
                MsgConnectionOpenInit, Version as ConnectionVersion,
            },
        },
        lightclients::solomachine::v1::{
            ClientState as SoloMachineClientState, ClientStateData, ConnectionStateData,
            ConsensusState as SoloMachineConsensusState, ConsensusStateData, DataType, SignBytes,
            TimestampedSignatureData,
        },
        lightclients::tendermint::v1::{
            ClientState as TendermintClientState, ConsensusState as TendermintConsensusState,
            Fraction,
        },
    },
};
use ibc::{
    core::{
        ics02_client::height::IHeight,
        ics07_tendermint::consensus_state::IConsensusState,
        ics23_vector_commitments::proof_specs,
        ics24_host::{
            identifier::{ChainId, ClientId, ConnectionId},
            path::{ClientStatePath, ConnectionPath, ConsensusStatePath},
        },
    },
    proto::{proto_encode, AnyConvert},
};
use k256::ecdsa::{signature::Signer, Signature};
use prost::Message;
use prost_types::Duration;
use tendermint::block::{Header, Height as BlockHeight};
use tendermint_light_client::{
    components::io::{AtHeight, Io, ProdIo},
    light_client::LightClient,
    state::State as LightClientState,
    store::{memory::MemoryStore, LightStore},
    types::Status,
};
use tendermint_rpc::Client;

use crate::{crypto::Crypto, handler::query_handler::QueryHandler, service::chain::Chain};

pub struct TransactionBuilder {
    chain: Chain,
    mnemonic: Mnemonic,
    memo: String,
}

impl TransactionBuilder {
    pub fn new(chain: Chain, mnemonic: Mnemonic, memo: String) -> Self {
        Self {
            chain,
            mnemonic,
            memo,
        }
    }

    /// Builds a transaction to create a solo machine client on IBC enabled chain
    pub async fn msg_create_solo_machine_client(&self) -> Result<TxRaw> {
        let any_public_key = self.mnemonic.to_public_key()?.to_any()?;

        let consensus_state = SoloMachineConsensusState {
            public_key: Some(any_public_key),
            diversifier: self.chain.diversifier.clone(),
            timestamp: self.chain.consensus_timestamp,
        };
        let any_consensus_state = consensus_state.to_any()?;

        let client_state = SoloMachineClientState {
            sequence: 1,
            frozen_sequence: 0,
            consensus_state: Some(consensus_state),
            allow_update_after_proposal: true,
        };
        let any_client_state = client_state.to_any()?;

        let message = MsgCreateClient {
            client_state: Some(any_client_state),
            consensus_state: Some(any_consensus_state),
            signer: self.mnemonic.account_address(&self.chain.account_prefix)?,
        };

        self.build(&[message]).await
    }

    /// Builds a transaction to create a tendermint client on IBC enabled solo machine
    pub async fn msg_create_tendermint_client<C>(
        &self,
        rpc_client: &C,
        light_client: &LightClient,
        light_client_io: &ProdIo,
    ) -> Result<(TendermintClientState, TendermintConsensusState)>
    where
        C: Client + Send + Sync,
    {
        let trust_level = Some(Fraction {
            numerator: *self.chain.trust_level.numer(),
            denominator: *self.chain.trust_level.denom(),
        });

        let unbonding_period = Some(self.get_unbonding_period().await?);
        let latest_height = self.get_latest_height(rpc_client).await?;

        let client_state = TendermintClientState {
            chain_id: self.chain.id.to_string(),
            trust_level,
            trusting_period: Some(self.chain.trusting_period.into()),
            unbonding_period,
            max_clock_drift: Some(self.chain.max_clock_drift.into()),
            frozen_height: Some(Height::zero()),
            latest_height: Some(latest_height.clone()),
            proof_specs: proof_specs(),
            upgrade_path: vec!["upgrade".to_string(), "upgradedIBCState".to_string()],
            allow_update_after_expiry: false,
            allow_update_after_misbehaviour: false,
        };

        let header = self.get_header(light_client, light_client_io, &latest_height)?;
        let consensus_state = TendermintConsensusState::from_block_header(header);

        Ok((client_state, consensus_state))
    }

    pub async fn msg_connection_open_init(
        &self,
        solo_machine_client_id: &ClientId,
        tendermint_client_id: &ClientId,
    ) -> Result<TxRaw> {
        let message = MsgConnectionOpenInit {
            client_id: solo_machine_client_id.to_string(),
            counterparty: Some(ConnectionCounterparty {
                client_id: tendermint_client_id.to_string(),
                connection_id: "".to_string(),
                prefix: Some(MerklePrefix {
                    key_prefix: "ibc".as_bytes().to_vec(),
                }),
            }),
            version: Some(ConnectionVersion {
                identifier: "1".to_string(),
                features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
            }),
            delay_period: 0,
            signer: self.mnemonic.account_address(&self.chain.account_prefix)?,
        };

        self.build(&[message]).await
    }

    pub async fn msg_connection_open_ack(
        &self,
        query_handler: &QueryHandler,
        solo_machine_connection_id: &ConnectionId,
        tendermint_client_id: &ClientId,
        tendermint_connection_id: &ConnectionId,
    ) -> Result<TxRaw> {
        let tendermint_client_state = query_handler
            .get_client_state(tendermint_client_id)?
            .ok_or_else(|| anyhow!("client for client id {} not found", tendermint_client_id))?;

        let message = MsgConnectionOpenAck {
            connection_id: solo_machine_connection_id.to_string(),
            counterparty_connection_id: tendermint_connection_id.to_string(),
            version: Some(ConnectionVersion {
                identifier: "1".to_string(),
                features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
            }),
            client_state: Some(tendermint_client_state.to_any()?),
            proof_height: Some(Height::new(0, 1)),
            proof_try: get_connection_proof(
                &self.chain,
                query_handler,
                &self.mnemonic,
                tendermint_connection_id,
            )?,
            proof_client: get_client_proof(
                &self.chain,
                query_handler,
                &self.mnemonic,
                &tendermint_client_id,
            )?,
            proof_consensus: get_consensus_proof(
                &self.chain,
                query_handler,
                &self.mnemonic,
                &tendermint_client_id,
            )?,
            consensus_height: Some(Height::new(0, 1)),
            signer: self.mnemonic.account_address(&self.chain.account_prefix)?,
        };

        self.build(&[message]).await
    }

    async fn build<T>(&self, messages: &[T]) -> Result<TxRaw>
    where
        T: AnyConvert,
    {
        let tx_body = self
            .build_tx_body(messages)
            .context("unable to build transaction body")?;
        let tx_body_bytes = proto_encode(&tx_body)?;

        let (account_number, account_sequence) = self.get_account_details().await?;

        let auth_info = self
            .build_auth_info(account_sequence)
            .context("unable to build auth info")?;
        let auth_info_bytes = proto_encode(&auth_info)?;

        let signature = self
            .build_signature(
                tx_body_bytes.clone(),
                auth_info_bytes.clone(),
                self.chain.id.to_string(),
                account_number,
            )
            .context("unable to sign transaction")?;

        Ok(TxRaw {
            body_bytes: tx_body_bytes,
            auth_info_bytes,
            signatures: vec![signature],
        })
    }

    fn build_tx_body<T>(&self, messages: &[T]) -> Result<TxBody>
    where
        T: AnyConvert,
    {
        let messages = messages
            .iter()
            .map(AnyConvert::to_any)
            .collect::<Result<_, _>>()?;

        Ok(TxBody {
            messages,
            memo: self.memo.clone(),
            timeout_height: 0,
            extension_options: Default::default(),
            non_critical_extension_options: Default::default(),
        })
    }

    fn build_auth_info(&self, account_sequence: u64) -> Result<AuthInfo> {
        let signer_info = SignerInfo {
            public_key: Some(self.mnemonic.to_public_key()?.to_any()?),
            mode_info: Some(ModeInfo {
                sum: Some(Sum::Single(Single { mode: 1 })),
            }),
            sequence: account_sequence,
        };

        let fee = Fee {
            amount: vec![Coin {
                denom: self.chain.fee.denom.clone(),
                amount: self.chain.fee.amount.to_string(),
            }],
            gas_limit: self.chain.fee.gas_limit,
            payer: "".to_owned(),
            granter: "".to_owned(),
        };

        Ok(AuthInfo {
            signer_infos: vec![signer_info],
            fee: Some(fee),
        })
    }

    fn build_signature(
        &self,
        body_bytes: Vec<u8>,
        auth_info_bytes: Vec<u8>,
        chain_id: String,
        account_number: u64,
    ) -> Result<Vec<u8>> {
        let sign_doc = SignDoc {
            body_bytes,
            auth_info_bytes,
            chain_id,
            account_number,
        };
        let sign_doc_bytes = proto_encode(&sign_doc)?;

        let signature: Signature = self.mnemonic.to_signing_key()?.sign(&sign_doc_bytes);
        Ok(signature.as_ref().to_vec())
    }

    async fn get_account_details(&self) -> Result<(u64, u64)> {
        let mut query_client = AuthQueryClient::connect(self.chain.grpc_addr.clone())
            .await
            .context(format!(
                "unable to connect to grpc query client at {}",
                self.chain.grpc_addr
            ))?;

        let account_address = self.mnemonic.account_address(&self.chain.account_prefix)?;

        let response = query_client
            .account(QueryAccountRequest {
                address: account_address.clone(),
            })
            .await?
            .into_inner()
            .account
            .ok_or_else(|| anyhow!("unable to find account with address: {}", account_address))?;

        let account = BaseAccount::decode(response.value.as_slice())?;

        Ok((account.account_number, account.sequence))
    }

    async fn get_unbonding_period(&self) -> Result<Duration> {
        let mut query_client = StakingQueryClient::connect(self.chain.grpc_addr.clone())
            .await
            .context(format!(
                "unable to connect to grpc query client at {}",
                self.chain.grpc_addr
            ))?;

        query_client
            .params(QueryParamsRequest::default())
            .await?
            .into_inner()
            .params
            .ok_or_else(|| anyhow!("staking params are empty"))?
            .unbonding_time
            .ok_or_else(|| anyhow!("missing unbonding period in staking params"))
    }

    async fn get_latest_height<C>(&self, rpc_client: &C) -> Result<Height>
    where
        C: Client + Send + Sync,
    {
        let response = rpc_client.status().await?;

        ensure!(
            !response.sync_info.catching_up,
            "node at {} running chain {} not caught up",
            self.chain.rpc_addr,
            self.chain.id,
        );

        let revision_number = response
            .node_info
            .network
            .as_str()
            .parse::<ChainId>()?
            .version();

        let revision_height = response.sync_info.latest_block_height.into();

        Ok(Height {
            revision_number,
            revision_height,
        })
    }

    fn get_header(
        &self,
        light_client: &LightClient,
        light_client_io: &ProdIo,
        height: &Height,
    ) -> Result<Header> {
        let height = height.to_block_height()?;
        let mut state = self.get_light_client_state(light_client_io, height)?;
        let light_block = light_client.verify_to_target(height, &mut state)?;

        Ok(light_block.signed_header.header)
    }

    fn get_light_client_state(
        &self,
        light_client_io: &ProdIo,
        height: BlockHeight,
    ) -> Result<LightClientState> {
        let trusted_block = light_client_io.fetch_light_block(AtHeight::At(height))?;

        let mut store = MemoryStore::new();
        store.insert(trusted_block, Status::Trusted);

        Ok(LightClientState::new(store))
    }
}

fn get_connection_proof(
    chain: &Chain,
    query_handler: &QueryHandler,
    mnemonic: &Mnemonic,
    connection_id: &ConnectionId,
) -> Result<Vec<u8>> {
    let connection = query_handler
        .get_connection(connection_id)?
        .ok_or_else(|| anyhow!("connection with id {} not found", connection_id))?;

    let mut connection_path = ConnectionPath::new(connection_id);
    connection_path.apply_prefix("ibc".parse().unwrap());

    let connection_state_data = ConnectionStateData {
        path: connection_path.into_bytes(),
        connection: Some(connection),
    };

    let connection_state_data_bytes = proto_encode(&connection_state_data)?;

    let sign_bytes = SignBytes {
        sequence: 1,
        timestamp: chain.consensus_timestamp,
        diversifier: chain.diversifier.to_owned(),
        data_type: DataType::ConnectionState.into(),
        data: connection_state_data_bytes,
    };

    sign(chain, mnemonic, sign_bytes)
}

fn get_client_proof(
    chain: &Chain,
    query_handler: &QueryHandler,
    mnemonic: &Mnemonic,
    client_id: &ClientId,
) -> Result<Vec<u8>> {
    let client_state = query_handler
        .get_client_state(client_id)?
        .ok_or_else(|| anyhow!("client with id {} not found", client_id))?
        .to_any()?;

    let mut client_state_path = ClientStatePath::new(client_id);
    client_state_path.apply_prefix("ibc".parse().unwrap());

    let client_state_data = ClientStateData {
        path: client_state_path.into_bytes(),
        client_state: Some(client_state),
    };

    let client_state_data_bytes = proto_encode(&client_state_data)?;

    let sign_bytes = SignBytes {
        sequence: 1,
        timestamp: chain.consensus_timestamp,
        diversifier: chain.diversifier.to_owned(),
        data_type: DataType::ClientState.into(),
        data: client_state_data_bytes,
    };

    sign(chain, mnemonic, sign_bytes)
}

fn get_consensus_proof(
    chain: &Chain,
    query_handler: &QueryHandler,
    mnemonic: &Mnemonic,
    client_id: &ClientId,
) -> Result<Vec<u8>> {
    let client_state = query_handler
        .get_client_state(client_id)?
        .ok_or_else(|| anyhow!("client with id {} not found", client_id))?;

    let height = client_state
        .latest_height
        .ok_or_else(|| anyhow!("client state does not contain latest height"))?;

    let consensus_state = query_handler
        .get_consensus_state(client_id, &height)?
        .ok_or_else(|| {
            anyhow!(
                "consensus state with id {} and height {} not found",
                client_id,
                height.to_string(),
            )
        })?
        .to_any()?;

    let mut consensus_state_path = ConsensusStatePath::new(client_id, &height);
    consensus_state_path.apply_prefix("ibc".parse().unwrap());

    let consensus_state_data = ConsensusStateData {
        path: consensus_state_path.into_bytes(),
        consensus_state: Some(consensus_state),
    };

    let consensus_state_data_bytes = proto_encode(&consensus_state_data)?;

    let sign_bytes = SignBytes {
        sequence: 1,
        timestamp: chain.consensus_timestamp,
        diversifier: chain.diversifier.to_owned(),
        data_type: DataType::ConsensusState.into(),
        data: consensus_state_data_bytes,
    };

    sign(chain, mnemonic, sign_bytes)
}

fn sign(chain: &Chain, mnemonic: &Mnemonic, sign_bytes: SignBytes) -> Result<Vec<u8>> {
    let sign_bytes = proto_encode(&sign_bytes)?;
    let signature: Signature = mnemonic.to_signing_key()?.sign(&sign_bytes);
    let signature_bytes: Vec<u8> = signature.as_ref().to_vec();

    let signature_data = SignatureData {
        sum: Some(SignatureDataInner::Single(SingleSignatureData {
            signature: signature_bytes,
            mode: SignMode::Direct.into(),
        })),
    };

    let signature_data_bytes = proto_encode(&signature_data)?;

    let timestamped_signature_data = TimestampedSignatureData {
        signature_data: signature_data_bytes,
        timestamp: chain.consensus_timestamp,
    };

    proto_encode(&timestamped_signature_data)
}
