#!/usr/bin/env bash

dataDir=$PWD/config
ledgerDir=$PWD/ledger

# For production, please change these variables to match the correct
# genesis hash, known validator, and entrypoints
#
GENESIS_HASH=${GENESIS_HASH:-"JDUh2fZyE8xLqSafQptssomfiutoPHQdzT242wMrhzXF"}
KNOWN_VALIDATOR=${KNOWN_VALIDATOR:-"8DBhFQTe1WsvWZYhHYv86ku4JM4AuWvxtZMRYBQHhHVg"}
ENTRYPOINT=${ENTRYPOINT:-"api.mainnet-internal.sonic.game:8001"}

validator_identity="$dataDir/validator-identity.json"
vote_account_identity="$dataDir/vote_account_identity.json"
secondary_validator_identity="$dataDir/secondary-validator-identity.json"

./bin/solana-validator \
  --identity $secondary_validator_identity \
  --authorized-voter $validator_identity \
  --vote-account $vote_account_identity \
  --known-validator $KNOWN_VALIDATOR \
  --only-known-rpc \
  --ledger $ledgerDir \
  --rpc-port 8899 \
  --dynamic-port-range 8000-8020 \
  --entrypoint $ENTRYPOINT \
  --expected-genesis-hash $GENESIS_HASH \
  --wal-recovery-mode skip_any_corrupted_record \
  --limit-ledger-size \
  --no-check-vote-account \
  --log $dataDir/logs/validator.log &
