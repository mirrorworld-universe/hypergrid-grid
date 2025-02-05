#!/usr/bin/env bash
#
# Run a minimal secondary failover node for existing cluster.  Ctrl-C to exit.
#
# Before running this script ensure standard Solana programs are available
# in the PATH, or that `cargo build` ran successfully
#
set -e

ok=true
for program in solana-{faucet,genesis,keygen,validator}; do
  $program -V || ok=false
done
$ok || {
  echo
  echo "Unable to locate required programs.  Try building them first with:"
  echo
  echo "  $ cargo build --all"
  echo
  exit 1
}

export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=debug} # if RUST_LOG is unset, default to info
export RUST_BACKTRACE=full
dataDir=$PWD/config
ledgerDir=$PWD/ledger

SOLANA_RUN_SH_CLUSTER_TYPE=${SOLANA_RUN_SH_CLUSTER_TYPE:-development}

set -x
if ! solana address; then
  echo Generating default keypair
  solana-keygen new --no-passphrase
fi

set -x
if ! solana address; then
  echo Generating default keypair
  solana-keygen new --no-passphrase
fi

validator_identity="$dataDir/validator-identity.json"
if [[ -e $validator_identity ]]; then
  echo "Use existing validator keypair"
else
  solana-keygen new --no-passphrase -so "$validator_identity"
fi

secondary_validator_identity="$dataDir/secondary-validator-identity.json"
if [[ -e $secondary_validator_identity ]]; then
  echo "Use existing secondary validator keypair"
else
  solana-keygen new --no-passphrase -so "$secondary_validator_identity"
fi
