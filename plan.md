# IBC Solo Machine

## IBC connection handshake

### Client creation

To build a `MsgCreateClient`, we need `client_state`, `consensus_state` and `signer`. We need to create a solo-machine
client on IBC enabled chain and a tendermint client on IBC enabled solo-machine.

1. To create a solo-machine client on IBC enabled chain, we first create `consensus_state`:

    ```rust
    let consensus_state = ConsensusState {
        public_key: "can be obtained from mnemonic provided by user",
        diversifier: "provided by user (can have a default value)",
        timestamp: "current timestamp"
    };
    ```

    Then, we create `client_state`:

    ```rust
    let client_state = ClientState {
        sequence: 1,
        frozen_sequence: 0,
        consensus_state,
        allow_update_after_proposal: false,
    };
    ```

    Lastly, `signer` can be obtained from mnemonic provided by user (`mnemonic.account_address()`).

    Finally, after creating `MsgCreateClient` from above values, build a raw transaction using `TransactionBuilder` and
    send it to tendermint using JSON RPC (`broadcast_tx_commit`).

    `client_id` can be extracted from response of `broadcast_tx_commit`: [code](https://github.com/devashishdxt/ibc-solo-machine/blob/c6cc022e42d67d059964f769a79511f6341e990a/src/relayer/rpc.rs#L67).

1. To create a tendermint client on IBC enabled solo-machine, we first create `client_state`:

    ```rust
    let client_state = ClientState {
        chain_id: "chain id of ibc enbled chain (provided by user)",
        trust_level: "provide by user (default value = 1/3)"
        trusting_period: "provided by user (default value = 336 * 60 * 60 = 336 hours)"
        unbonding_period: "unbonding period of ibc enabled chain (can be obtained from grpc query, see notes below)",
        max_clock_drift: "provide by user (default value = 3000 millis)",
        frozen_height: 0,
        latest_height: "latest height of ibc enabled chain (can be obtained from rpc query, see notes below)",
        proof_specs: "cosmos sdk specs (see notes below)",
        upgrade_path: vec!["upgrade".to_string(), "upgradedIBCState".to_string()],
        allow_update_after_expiry: false,
        allow_update_after_misbehaviour: false,
    };
    ```

    > Note:
    > 
    > 1. `unbonding_period`: [grpc query](https://docs.rs/ibc-proto/0.7.1/ibc_proto/cosmos/staking/v1beta1/query_client/struct.QueryClient.html#method.params),
    > [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain/cosmos.rs#L88)
    > 1. `latest_height`: [rpc query](https://docs.rs/tendermint-rpc/0.19.0/tendermint_rpc/trait.Client.html#method.status),
    > [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain/cosmos.rs#L457)
    > 1. `proof_specs`: [code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/core/23-commitment/types/merkle.go#L17)

    Then we create `consensus_state` (needs tendermint light client): [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/foreign_client.rs#L243),
    [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain/runtime.rs#L405),
    [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain/cosmos.rs#L1117),
    [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/modules/src/ics07_tendermint/consensus_state.rs#L90)

    Lastly, `signer` can be obtained from mnemonic provided by user (`mnemonic.account_address()`).

    Finally, pass these values to `IbcTxHandler` of solo-machine.

1. After `IbcTxHandler` on solo-machine recieves `client_state` and `consensus_state`: [code](https://github.com/devashishdxt/ibc/blob/55786f270350c57ce691a1ba8b420aefd63de63c/solo-machine/src/ibc_handler.rs#L26)

    - Generate a `client_id`
    - Store `client_state` in `private_store`
    - Store `consensus_state` in `provable_store`

    > Implementation note:
    >
    > For solo machine, we do not need two different stores, i.e., `private_store` and `provable_store`. Current
    > implementation only uses one store (non-provable).

### Connection creation

To establish a connection between IBC enabled chain and solo-machine, we need to send multiple messages to both, IBC
enabled chain and solo-machine ([specs](https://github.com/cosmos/ibc/tree/master/spec/core/ics-003-connection-semantics#opening-handshake)). In a nutshell:

1. Send `MsgConnectionOpenInit` to IBC enabled chain
1. Send `MsgConnectionOpenTry` to IBC enabled solo-machine
1. Send `MsgConnectionOpenAck` to IBC enabled chain
1. Send `MsgConnectionOpenConfirm` to IBC enabled solo-machine

To establish connection on both sides:

1. `MsgConnectionOpenInit`: [code](https://github.com/devashishdxt/ibc-solo-machine/blob/c6cc022e42d67d059964f769a79511f6341e990a/src/transaction_builder.rs#L104)

    First, create `MsgConnectionOpenInit`:

    ```rust
    let msg = MsgConnectionOpenInit {
        client_id: "client id of solo-machine client on ibc enabled chain (client id returned from 1st step of client creation)",
        counterparty: Counterparty {
            client_id: "client id of tendermint client on solo machine (client id returned from 2nd step of client creation)",
            connection_id: "".to_string(),
            prefix: MerklePrefix {
                key_prefix: "ibc".to_string(), // this should be obtained from a real chain query (ok for now)
            }
        },
        version: Version {
            identifier: "1".to_string(),
            features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
        },
        delay_period: 0,
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())"
    };
    ```

    Next, build a raw transaction using `TransactionBuilder` and send it to tendermint using JSON RPC (`broadcast_tx_commit`).

    `connection_id` can be extracted from response of `broadcast_tx_commit`: [code](https://github.com/devashishdxt/ibc-solo-machine/blob/c6cc022e42d67d059964f769a79511f6341e990a/src/relayer/rpc.rs#L86)

1. `MsgConnectionOpenTry`:

    First, create `MsgConnectionOpenTry`:

    ```rust
    let msg = MsgConnectionOpenTry {
        client_id: "client id of tendermint client on solo machine (client id returned from 2nd step of client creation)",
        previous_connection_id: "".to_string(),
        client_state: "client state of solo-machine client stored on ibc-enabled chain (obtained by rpc query, see notes below)",
        counterparty: Counterparty {
            client_id: "client id of solo-machine client on ibc enabled chain (client id returned from 1st step of client creation)",
            connection_id: "connection id of solo machine on ibc enabled chain (connection id returned from `MsgConnectionOpenInit`)",
            prefix: MerklePrefix {
                key_prefix: "ibc".to_string(), // this should be obtained from a real chain query (ok for now)
            },
        },
        delay_period: 0,
        counterparty_versions: vec![Version {
            identifier: "1".to_string(),
            features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
        }],
        proof_height: "block height for which the proofs were queried (obtained by rpc query, see notes below)",
        proof_init: "obtained by rpc query, see notes below",
        proof_client: "obtained by rpc query, see notes below",
        proof_consensus: "obtained by rpc query, see notes below",
        consensus_height: proof_consensus.height(),
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    > Note:
    >
    > 1. `client_state`, `proof_init`, `client_proof`, `consensus_proof` and `proof_height`: [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain.rs#L264)

    Next, pass all the above values to `IbcTxHandler` of solo-machine.

    > Important:
    >
    > Before sending `MsgConnectionOpenTry` to `IbcTxHandler`, we need to update tendermint client on solo-machine to the
    > target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/connection.rs#L518)

    After `IbcTxHandler` receives `MsgConnectionOpenTry`:

    - Generate a new `connection_id` (if there is a `previous_connection_id`, we need to do some extra step which are
      not done in current implementation)
    - Fetch current `block_height` of solo machine (`current_block_height`) and return error if `consensus_height` is
      greater than or equal to `current_block_height`.
    - 

1. `MsgConnectionOpenAck`:

    First, create `MsgConnectionOpenAck`:

    ```rust
    let msg = MsgConnectionOpenAck {
        connection_id: "connection id of solo machine client on ibc enabled chain (connection id returned from `MsgConnectionOpenInit`)",
        counterparty_connection_id: "connection id of tendermint client on ibc enabled solo machine (connection id returned for `MsgConnectionOpenTry` from `IbcTxHandler`)",
        version: Version {
            identifier: "1".to_string(),
            features: vec!["ORDER_ORDERED".to_string(), "ORDER_UNORDERED".to_string()],
        },
        client_state: "client state of ibc enabled chain stored on solo machine (can be obtained from `IbcQueryHandler`)"
        proof_height: "block height of solo machine for which the proofs were constructed (see notes below)",
        proof_try: "see notes below",
        proof_client: "see notes below",
        proof_consensus: "see notes below",
        consensus_height: "latest height of ibc enabled chain's consensus state stored on solo machine",
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())", 
    };
    ```

    > Note:
    >
    > 1. `proof_try`: Signature obtained by signing [`SignBytes`](https://github.com/devashishdxt/ibc-solo-machine/blob/ics-impl/src/ics/solo_machine_client/sign_bytes.rs)
    > with `signing_key` obtained from `ConnectionStateData`. [proof verification code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/client_state.go#L178),
    > [code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/proof.go#L197)
    > 1. `proof_client`: Signature obtained by signing [`SignBytes`](https://github.com/devashishdxt/ibc-solo-machine/blob/ics-impl/src/ics/solo_machine_client/sign_bytes.rs)
    > with `signing_key` obtained from `ClientStateData`. [proof verification code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/client_state.go#L178)
    > 1. `proof_consensus`: Signature obtained by signing [`SignBytes`](https://github.com/devashishdxt/ibc-solo-machine/blob/ics-impl/src/ics/solo_machine_client/sign_bytes.rs)
    > with `signing_key` obtained from `ConsensusStateData`. [proof verification code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/client_state.go#L140)


    Next, build a raw transaction using `TransactionBuilder` and send it to tendermint using JSON RPC (`broadcast_tx_commit`).

    > Important:
    >
    > Before sending `MsgConnectionOpenAck` to IBC enabled chain, we need to update solo-machine client on IBC enabled
    > chain to the target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/connection.rs#L635)

1. `MsgConnectionOpenConfirm`:

    First, create `MsgConnectionOpenConfirm`:

    ```rust
    let msg = MsgConnectionOpenConfirm {
        connection_id: "connection id of tendermint client on ibc enabled solo-machine",
        proof_ack: "obtained by rpc query, see notes below",
        proof_height: "block height for which the proofs were queried (obtained by rpc query, see notes below)",
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    > Note:
    >
    > 1. `proof_ack` and `proof_height`: Refer to the same code to generate proofs as in `MsgConnectionOpenTry`.

    Next, pass all the above values to `IbcTxHandler` of solo-machine.

    > Important:
    >
    > Before sending `MsgConnectionOpenTry` to `IbcTxHandler`, we need to update tendermint client on solo-machine to the
    > target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/connection.rs#L725)

### Channel creation

To create a channel between IBC enabled chain and solo-machine, we need to send multiple messages to both, IBC enabled
chain and solo-machine ([specs](https://github.com/cosmos/ibc/tree/master/spec/core/ics-003-connection-semantics#opening-handshake)).
In a nutshell:

1. Send `MsgChannelOpenInit` to IBC enabled chain
1. Send `MsgChannelOpenTry` to IBC enabled solo-machine
1. Send `MsgChannelOpenAck` to IBC enabled chain
1. Send `MsgChannelOpenConfirm` to IBC enabled solo-machine

To create a channel on both sides:

1. `MsgChannelOpenInit`:

    First, create `MsgChannelOpenInit`:

    ```rust
    let msg = MsgChannelOpenInit {
        port_id: "port id of channel on ibc enabled chain (provided by user, autogenerate?, default value = transfer)",
        channel: Channel {
            state: Init,
            ordering: "provided by user (default value = Unordered)",
            counterparty: Counterparty {
                port_id: "port id of channel on ibc enabled solo machine (provided by user, autogenerate?)",
                channel_id: "".to_string(),
            },
            connection_hops: vec!["connection id of solo machine on ibc enabled chain (connection id returned from `MsgConnectionOpenInit`)"],
            version: "can be fetched from query (currently not possible, default value = 'ics20-1', see notes below)"
        },
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    > Note:
    >
    > `port_id` can be provided by user and `version` can be fetched from grpc query. But, currently, it is not possible.
    > So, it is best to hardcode the values for now. [value ref](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/apps/transfer/types/keys.go#L16)

    Next, build a raw transaction using `TransactionBuilder` and send it to tendermint using JSON RPC (`broadcast_tx_commit`).

    `channel_id` can be extracted from response of `broadcast_tx_commit`: [sample attribute extraction code](https://github.com/devashishdxt/ibc-solo-machine/blob/c6cc022e42d67d059964f769a79511f6341e990a/src/relayer/rpc.rs#L86),
    [event structure](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/core/04-channel/handler.go#L22)

1. `MsgChannelOpenTry`:

    First, create `MsgChannelOpenTry`:

    ```rust
    let msg = MsgChannelOpenTry {
        port_id: "port id of channel on ibc enabled solo machine (provided by user, autogenerate?, default value = transfer)",
        previous_channel_id: "".to_string(),
        channel: Channel {
            state: TryOpen,
            ordering: "provided by user (default value = Unordered)",
            counterparty: Counterparty {
                port_id: "port id of channel on ibc enabled solo machine (provided by user, autogenerate?)",
                channel_id: "channel id of solo machine on ibc enabled chain (channel id returned from `MsgChannelOpenInit`)"
            },
            connection_hops: vec!["connection id of tendermint client on ibc enabled solo-machine (connection id returned from `MsgConnectionOpenTry`)"],
            version: "can be fetched from `IbcQueryHandler` (default value = 'ics20-1', see notes below)",
        },
        counterparty_version: "can be fetched from query (currently not possible, default value = 'ics20-1', see notes below)",
        proof_init: "obtained by rpc query, see notes below",
        proof_height: "block height for which the proofs were queried (obtained by rpc query, see notes below)",
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    > Note:
    >
    > 1. `port_id` can be provided by user and `version` can be fetched from grpc query. But, currently, it is not
    > possible. So, it is best to hardcode the values for now. [value ref](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/apps/transfer/types/keys.go#L16)
    > 1. `proof_init` and `proof_height`: [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/chain.rs#L344)

    Next, pass all the above values to `IbcTxHandler` of solo-machine.

    > Important:
    >
    > Before sending `MsgChannelOpenTry` to `IbcTxHandler`, we need to update tendermint client on solo-machine to the
    > target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/channel.rs#L457)

1. `MsgChannelOpenAck`

    First, create `MsgChannelOpenAck`:

    ```rust
    let msg = MsgChannelOpenAck {
        port_id: "port id of channel on ibc enabled chain (provided by user, autogenerate?, default value = transfer)",
        channel_id: "channel id of solo machine on ibc enabled chain (channel id returned from `MsgChannelOpenInit`)",
        counterparty_channel_id: "channel id of tendermint client on ibc enabled solo machine (channel id returned for `MsgChannelOpenTry` from `IbcTxHandler`)",
        counterparty_version: "ics20-1".to_string(),
        proof_try: "see notes below",
        proof_height: "block height of solo machine for which the proofs were constructed (see notes below)",
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    > Note:
    >
    > 1. `proof_try`: Signature obtained by signing [`SignBytes`](https://github.com/devashishdxt/ibc-solo-machine/blob/ics-impl/src/ics/solo_machine_client/sign_bytes.rs)
    > with `signing_key` obtained from `ChannelStateData`. [proof verification code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/client_state.go#L215).
    > [code](https://github.com/cosmos/ibc-go/blob/58df13ca2847ed42a18666b337a4bd8ec0aac1eb/modules/light-clients/06-solomachine/types/proof.go#L250)

    Next, build a raw transaction using `TransactionBuilder` and send it to tendermint using JSON RPC (`broadcast_tx_commit`).

    > Important:
    >
    > Before sending `MsgChannelOpenAck` to IBC enabled chain, we need to update solo-machine client on IBC enabled
    > chain to the target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/channel.rs#L565)

1. `MsgChannelOpenConfirm`

    First, create `MsgChannelOpenConfirm`:

    ```rust
    let msg = MsgChannelOpenConfirm {
        port_id: "port id of channel on ibc enabled solo machine (provided by user, autogenerate?, default value = transfer)",
        channel_id: "channel id of tendermint client on ibc enabled solo machine (channel id returned for `MsgChannelOpenTry` from `IbcTxHandler`)",
        proof_ack: "obtained by rpc query, see notes below",
        proof_height: "block height for which the proofs were queried (obtained by rpc query, see notes below)",
        signer: "can be obtained from mnemonic provided by user (mnemonic.account_address())",
    };
    ```

    Next, pass all the above values to `IbcTxHandler` of solo-machine.

    > Important:
    >
    > Before sending `MsgChannelOpenConfirm` to `IbcTxHandler`, we need to update tendermint client on solo-machine to the
    > target height (`proof_height`) using `MsgUpdateClient`. [code](https://github.com/informalsystems/ibc-rs/blob/1c72c281939e37187919161ab8791cebd3d572e7/relayer/src/channel.rs#L649)
