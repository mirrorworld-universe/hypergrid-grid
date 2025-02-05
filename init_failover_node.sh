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

vote_account_identity="$dataDir/vote_account_identity.json"
if [[ -e $vote_account_identity ]]; then
  echo "Use existing vote account keypair"
else
  solana-keygen new --no-passphrase -so "$vote_account_identity"
fi

validator_stake_account="$dataDir/validator-stake-account.json"
if [[ -e $validator_stake_account ]]; then
  echo "Use existing validator stake account keypair"
else
  solana-keygen new --no-passphrase -so "$validator_stake_account"
fi

authorized_withdrawer_identity="$dataDir/authorized-withdrawer-identity.json"
if [[ -e $authorized_withdrawer_identity ]]; then
  echo "Use existing authorized withdrawal identity keypair"
else
  solana-keygen new --no-passphrase -so "$authorized_withdrawer_identity"
fi

# Only create the vote account if it doesn't already exist

# ./bin/solana create-vote-account -ut \
#   --fee-payer $secondary_validator_identity \
#   $vote_account_identity \
#   $secondary_validator_identity \
#   $authorized_withdrawer_identity
