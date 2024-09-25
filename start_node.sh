export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
dataDir=$PWD/config
echo $dataDir

./bin/solana-validator \
	--identity $dataDir/keys/validator-identity.json \
	--vote-account $dataDir/keys/validator-vote-account.json \
	--ledger $dataDir/ledger \
	--gossip-port 8001 \
	--full-rpc-api \
	--rpc-port 8899 \
	--rpc-bind-address 0.0.0.0 \
	--rpc-faucet-address 127.0.0.1:9900 \
	--enable-rpc-transaction-history \
	--enable-extended-tx-metadata-storage \
	--init-complete-file $dataDir/keys/init-completed \
	--require-tower \
	--no-wait-for-vote-to-start-leader \
	--no-os-network-limits-test \
	--rpc-pubsub-enable-block-subscription \
	--rpc-pubsub-enable-vote-subscription \
	--rpc-threads 16 \
	--account-index program-id \
	--account-index spl-token-owner \
	--account-index spl-token-mint \
	--accounts-db-cache-limit-mb 20240 \
	--accounts-index-memory-limit-mb 4096 \
	--accounts-index-scan-results-limit-mb 4096 \
	--limit-ledger-size 2000000000 \
	--log $dataDir/logs/validator.log &

# 	--gossip-host 52.10.174.63 \