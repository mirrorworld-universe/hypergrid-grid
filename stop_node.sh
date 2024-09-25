#kill -15 `ps -aux|grep solana-validator | grep -v grep | awk '{print $2}'`
export RUST_LOG=${RUST_LOG:-solana=info,solana_runtime::message_processor=info,solana_metrics::metrics=warn}
export RUST_BACKTRACE=full
dataDir=$PWD/config
./bin/solana-validator \
	--ledger $dataDir/ledger \
	exit --force
