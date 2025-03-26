# Configuration and Usage

As outlined in the [Architecture](./architecture.md) chapter, the node consists of several components which can be run
separately or as a single bundled process. At present, the recommended way to operate a node is in bundled mode and is
what this guide will focus on. Operating the components separately is very similar and should be relatively
straight-foward to derive from these instructions.

This guide focusses on basic usage. To discover more advanced options we recommend exploring the various help menus
which can be accessed by appending `--help` to any of the commands.

## Bootstrapping

The first step in starting a new Miden network is to initialize the genesis block data. This is a once-off operation.

```sh
# Write the default genesis configuration to a file.
#
# You can customize this file to add or remove accounts from the genesis block.
# By default this includes a single public faucet account.
#
# This can be skipped if using the default configuration.
miden-node store dump-genesis > genesis.toml

# Create a folder to store the node's data.
mkdir data 

# Create a folder to store the genesis block's account secrets and data.
#
# These can be used to access the accounts afterwards.
# Without these the accounts would be inaccessible.
mkdir accounts

# Bootstrap the node.
#
# This generates the genesis data and stores it in `<data-directory>/genesis.dat`.
# This is used by the node to create and verify the database during node startup.
#
# Account secrets are stored as `<accounts-directory>/account_xx.mac`
# where `xx` is the index of the account in the configuration file.
#
# These account files are not used by the node and should instead be used wherever
# you intend to operate these accounts,
# e.g. to run the `miden-faucet` (see Faucet section).
miden-node bundled bootstrap \
  --data-directory data \
  --accounts-directory accounts \
  --config genesis.toml  # This can be omitted to use the default config.
```

## Operation

Start the node with the desired public gRPC server address.

```sh
miden-node bundled start \
  --data-directory data \
  --rpc.url http://0.0.0.0:57123
```

## Faucet

We also provide a reference implementation for a public faucet app with a basic webinterface to request
tokens. The app requires a faucet account file which it can either generate (for a new account), or it can use an
existing one e.g. one created as part of the genesis block.

Create a faucet account for the faucet app to use - or skip this step if you already have an account file.

```sh
mkdir accounts
miden-faucet create-faucet-account \
  --token-symbol MY_TOKEN \
  --decimals 12 \
  --max-supply 5000
```

Create a configuration file for the faucet.  

```sh
# This generates `miden-faucet.toml` which is used to configure the faucet.
#
# You can inspect and modify this if you want to make changes, e.g. to the website url.
miden-faucet init \
  --config-path miden-faucet.toml \
  --faucet-account-path accounts/account_0.mac # Filename may be different if you created a new account.
```

Run the faucet:

```sh
miden-faucet --config miden-faucet.toml
```

## Systemd

Our [Debian packages](./installation.md#debian-package) install a systemd service which operates the node in `bundled`
mode. You'll still need to run the [bootstrapping](#bootstrapping) process before the node can be started.

You can inspect the service file with `systemctl cat miden-node` (and `miden-faucet`) or alternatively you can see it in
our repository in the `packaging` folder. For the bootstrapping process be sure to specify the data-directory as
expected by the systemd file. If you're operating a faucet from an account generated in the genesis block, then you'll
also want to specify the accounts directory as expected by the faucet service file. With the default unmodified service
files this would be:

```sh
miden-node bundled bootstrap \
  --data-directory /opt/miden-node \
  --accounts-directory /opt/miden-faucet
```

## Environment variables

Most configuration options can also be configured using environment variables as an alternative to providing the values
via the command-line. This is useful for certain deployment options like `docker` or `systemd`, where they can be easier
to define or inject instead of changing the underlying command line options.

These are especially convenient where multiple different configuration profiles are used. Write the environment
variables to some specific `profile.env` file and load it as part of the node command:

```sh
source profile.env && miden-node <...>
```

This works well on Linux and MacOS, but Windows requires some additional scripting unfortunately.
