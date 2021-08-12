# IBC Solo Machine

This repository implements IBC solo machine which can be used to interface with other machines & replicated ledgers
which speak IBC.

## Building

To build `solo-machine` binary, run: `cargo build  --package solo-machine`.

## Usage

Solo machine CLI has following sub-commands:

```
solo-machine-cli 0.1.0
A command line interface for IBC solo machine

USAGE:
    solo-machine [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help        Prints help information
        --no-style    Does not print styled/colored statements
    -V, --version     Prints version information

OPTIONS:
        --account-prefix <account-prefix>    Prefix for bech32 addresses used on solo machine [env: SOLO_ACCOUNT_PREFIX]
        --address-algo <address-algo>        Algoritm to use for generating addresses [env: SOLO_ADDRESS_ALGO]
                                             [default: secp256k1]  [possible values: secp256k1, eth-secp256k1]
        --db-path <db-path>                  Database connection string [env: SOLO_DB_PATH]
        --handler <handler>...               Register an event handler. Multiple event handlers can be registered and
                                             they're executed in order they're provided in CLI. Also, if an event
                                             handler returns an error when handling a message, all the future event
                                             handlers will not get executed
        --hd-path <hd-path>                  HD wallet path to be used when deriving public key from mnemonic [env:
                                             SOLO_HD_PATH]  [default: m/44'/118'/0'/0/0]
        --mnemonic <mnemonic>                Mnemonic phrase for account on IBC enabled chain [env: SOLO_MNEMONIC]

SUBCOMMANDS:
    chain             Chain operations (managing chain state and metadata)
    gen-completion    Generate completion scripts for solo-machine-cli
    help              Prints this message or the help of the given subcommand(s)
    ibc               Used to connect, mint tokens and burn tokens on IBC enabled chain
    init              Initializes database for solo machine
    start             Starts gRPC server for solo machine
```

- `chain` sub-command is used to manage an IBC enabled chain's state and metadata on solo machine, for example, its
  gRPC address, fee configuration, etc.
- `ibc` sub-command is used to broadcast IBC related transactions to cosmos SDK chain. This includes `connect`, `mint`
  (mint tokens on cosmos SDK chain) and `burn` (burn tokens on cosmos SDK chain).

Other than these three core commands,

- `init` is used to initialize SQLite database at given location.
- `start` is used to start a gRPC server which has endpoints for all the above three core functions.
- `gen-completion` generates autocompletion scripts for different shells.

In addition to these sub-commands, solo machine also has some configuration options which can either be provided using
command line options, environment variables or in a `.env` file.

### Connecting to a Cosmos SDK chain

To connect to a cosmos SDK chain, we first need an account on cosmos SDK chain with enough tokens so that it can pay
transaction fee for IBC transactions.

1. Create a `.env` file with `SOLO_ACCOUNT_PREFIX`, `SOLO_DB_PATH`, `SOLO_MNEMONIC` and `SOLO_HD_PATH` (if it is
   different from the default one).
2. Run `solo-machine init` to initialize SQLite database.
3. Add cosmos SDK chain details using `solo-machine chain add`. This command takes following options which can either be
   provided using command line options, environment variables or a `.env` file. The two most important things are
   `trusted-height` and `trusted-hash` which can be fetched from:
   `curl http://<ip>:<port>/block?height=<trusted-height>`.

   ```
   solo-machine-chain-add 0.1.0
   Adds metadata for new IBC enabled chain
   
   USAGE:
       solo-machine chain add [OPTIONS] --trusted-hash <trusted-hash> --trusted-height <trusted-height>
   
   FLAGS:
       -h, --help       Prints help information
       -V, --version    Prints version information
   
   OPTIONS:
           --diversifier <diversifier>            Diversifier used in transactions for chain [env: SOLO_DIVERSIFIER]
                                                  [default: solo-machine-diversifier]
           --fee-amount <fee-amount>              Fee amount [env: SOLO_FEE_AMOUNT]  [default: 1000]
           --fee-denom <fee-denom>                Fee denom [env: SOLO_FEE_DENOM]  [default: stake]
           --gas-limit <gas-limit>                Gas limit [env: SOLO_GAS_LIMIT]  [default: 300000]
           --grpc-addr <grpc-addr>                gRPC address of IBC enabled chain [env: SOLO_GRPC_ADDRESS]  [default:
                                                  http://0.0.0.0:9090]
           --max-clock-drift <max-clock-drift>    Maximum clock drift [env: SOLO_MAX_CLOCK_DRIFT]  [default: 3 sec]
           --port-id <port-id>                    Port ID used to create connection with chain [env: SOLO_PORT_ID]
                                                  [default: transfer]
           --rpc-addr <rpc-addr>                  RPC address of IBC enabled chain [env: SOLO_RPC_ADDRESS]  [default:
                                                  http://0.0.0.0:26657]
           --rpc-timeout <rpc-timeout>            RPC timeout duration [env: SOLO_RPC_TIMEOUT]  [default: 60 sec]
           --trust-level <trust-level>            Trust level (e.g. 1/3) [env: SOLO_TRUST_LEVEL]  [default: 1/3]
           --trusted-hash <trusted-hash>          Block hash at trusted height of the chain [env: SOLO_TRUSTED_HASH]
           --trusted-height <trusted-height>      Trusted height of the chain [env: SOLO_TRUSTED_HEIGHT]
           --trusting-period <trusting-period>    Trusting period [env: SOLO_TRUSTING_PERIOD]  [default: 14 days]
   ```

4. Establish IBC connection with the chain using `solo-machine ibc connect <chain-id>`.
5. Mint tokens on cosmos SDK chain using `solo-machine ibc mint <chain-id> <amount> <denom>`.
6. Burn some tokens on cosmos SDK chain using `solo-machine ibc burn <chain-id> <amount> <denom>`. Note that the
   `denom` in `burn` command will be the denom on solo machine and not the IBC denom (`ibc/XXX`).

### Connecting to Ethermint

If you wish to connect to ethermint using solo machine, you'll have to enable `ethermint` feature when building:
`cargo build --package solo-machine --features ethermint` and also provide `SOLO_ADDRESS_ALGO="eth-secp256k1"` in `.env`
file if you're using native `eth-secp256k1` addresses on ethermint.

### Adding event hooks

Solo machine supports adding event hooks at runtime using dynamic libraries (`dylib`). To create a new event hook, the
dynamic library should expose a function named `register_handler` with signature:

```rust
fn register_handler(registrar: &mut dyn HandlerRegistrar)
```

The implementation of `register_handler` can call `registrar.register()` and pass a `Box`ed object of `EventHandler`. A
sample event hook can be found [here](event-hooks/stdout-logger) and can be used as a template to develop more complex
event hooks.

Note that in `Cargo.toml`, we have to add following lines to make it a dynamic library.

```toml
[lib]
crate-type = ["dylib"]
```

Once implemented, the library can be compiled to `*.so` file and supplied to solo machine using `--handler` CLI option.

For example,

```
solo-machine --handler="<path-to-dylib-.so-file>" ibc mint 100 gld
```

All the events that can be generated by solo machine can be found [here](solo-machine-core/src/event.rs).

## License

Licensed under Apache License, Version 2.0 ([LICENSE](LICENSE)).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as
defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
