#!/bin/bash
export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
export SOLANA_RUN_SH_CLUSTER_TYPE=mainnet-beta
export SONIC_FEE_MULTIPLIER=5000
dataDir=$PWD

./bin/solana-validator \
	--identity $dataDir/config/validator-identity.json \
	--vote-account $dataDir/config/validator-vote-account.json \
	--ledger $dataDir/ledger \
	--gossip-port 8001 \
	--gossip-host <YOUR_PUBLIC_IP> \
	--full-rpc-api \
	--rpc-port 8899 \
	--public-rpc-address <YOUR_PUBLIC_IP>:8899 \
	--rpc-bind-address 0.0.0.0 \
	--enable-rpc-transaction-history \
	--enable-extended-tx-metadata-storage \
	--init-complete-file $dataDir/config/init-completed \
	--require-tower \
	--no-wait-for-vote-to-start-leader \
	--no-os-network-limits-test \
	--rpc-pubsub-enable-block-subscription \
	--rpc-pubsub-enable-vote-subscription \
	--rpc-threads 128 \
	--account-index program-id \
	--account-index spl-token-owner \
	--account-index spl-token-mint \
	--accounts-db-cache-limit-mb 819200 \
	--accounts-index-memory-limit-mb 10240 \
	--accounts-index-scan-results-limit-mb 10240 \
	--limit-ledger-size 50000000 \
	--log $dataDir/logs/validator.log &
