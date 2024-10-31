export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
export SONIC_FEE_MULTIPLIER=5000
dataDir=$PWD
./bin/solana-validator \
    --identity $dataDir/config/validator-keypair.json \
    --known-validator Ci2TRaVpJoNmTUVhw5tkTVnskXVJ7FQHCMKRFGrQSHJB \
	--repair-validator Ci2TRaVpJoNmTUVhw5tkTVnskXVJ7FQHCMKRFGrQSHJB \
    --ledger $dataDir/ledger \
	--only-known-rpc \
	--no-voting \
    --entrypoint 52.13.90.86:8001 \
	--gossip-host <YOUR_PUBLIC_IP> \
	--gossip-port 8001 \
	--rpc-port 8899 \
    --full-rpc-api \
    --rpc-bind-address 0.0.0.0 \
	--public-rpc-address <YOUR_PUBLIC_IP>:8899 \
	--enable-rpc-transaction-history \
	--enable-extended-tx-metadata-storage \
	--no-wait-for-vote-to-start-leader \
	--no-os-network-limits-test \
	--rpc-pubsub-enable-block-subscription \
	--rpc-pubsub-enable-vote-subscription \
	--account-index program-id \
	--account-index spl-token-owner \
	--account-index spl-token-mint \
	--rpc-threads 128 \
	--accounts-db-cache-limit-mb 819200 \
	--accounts-index-memory-limit-mb 81920 \
	--accounts-index-scan-results-limit-mb 81920 \
	--limit-ledger-size 500000000 \
    --expected-genesis-hash E8nY8PG8PEdzANRsv91C2w28Dbw9w3AhLqRYfn5tNv2C \
    --wal-recovery-mode skip_any_corrupted_record \
	--log $dataDir/logs/validator.log &
