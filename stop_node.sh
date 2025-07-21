#!/bin/bash
set -e

export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
dataDir=$PWD
./bin/solana-validator \
	--ledger $dataDir/ledger \
	exit --force
