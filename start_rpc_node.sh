RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
export SOLANA_RUN_SH_CLUSTER_TYPE=mainnet-beta
export SONIC_FEE_MULTIPLIER=5000

./bin/solana-validator \
    --identity ./config/validator-keypair.json \
    --known-validator VALIDATOR_ID \
    --repair-validator VALIDATOR_ID \
    --ledger ./ledger \
    --rpc-port 8899 \
    --full-rpc-api \
    --no-voting \
    --rpc-bind-address 0.0.0.0 \
    --only-known-rpc \
    --gossip-host YOUR_PUBLIC_IP \
    --gossip-port 8001 \
    --entrypoint VALIDATOR_PUBLIC_IP:8001 \
    --public-rpc-address YOUR_PUBLIC_IP:8899 \
    --enable-rpc-transaction-history \
    --enable-extended-tx-metadata-storage \
    --no-wait-for-vote-to-start-leader \
    --no-os-network-limits-test \
    --rpc-pubsub-enable-block-subscription \
    --rpc-pubsub-enable-vote-subscription \
    --account-index program-id \
    --account-index spl-token-owner \
    --account-index spl-token-mint \
    --accounts-db-cache-limit-mb 102400 \
    --accounts-index-memory-limit-mb 40960 \
    --accounts-index-scan-results-limit-mb 40960 \
    --limit-ledger-size 500000000 \
    --expected-genesis-hash VALIDATOR_GENESIS_HASH \
    --wal-recovery-mode skip_any_corrupted_record \
    --log ./logs/validator.log &
