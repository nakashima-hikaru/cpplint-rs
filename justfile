
QUANTLIB_DIR := "bench_data/QuantLib"
QUANTLIB_URL := "https://github.com/lballabio/QuantLib.git"

# QuantLibをcloneして、Criterionによるベンチマークを実行します
bench-quantlib: clone-quantlib
	@echo "Running Criterion benchmark for QuantLib..."
	cargo bench -p cpplint-core --bench quantlib

# QuantLibのリポジトリをcloneします (存在しない場合のみ)
clone-quantlib:
	@if [ ! -d "{{QUANTLIB_DIR}}" ]; then \
		echo "Cloning QuantLib into {{QUANTLIB_DIR}}..."; \
		mkdir -p bench_data; \
		git clone --depth 1 {{QUANTLIB_URL}} {{QUANTLIB_DIR}} --quiet; \
	fi

# リリースモードで実行バイナリをビルドします
build-release:
	@echo "Building cpplint-rs in release mode..."
	@cargo build --release --quiet

# QuantLibに対するメモリ使用量と実行時間を計測します
measure-quantlib: clone-quantlib build-release
	@echo "Measuring memory usage and time for QuantLib..."
	@(/usr/bin/time -l ./target/release/cpplint --recursive {{QUANTLIB_DIR}} > /dev/null) 2>&1 | tail -n 20

# ベンチマーク用のデータをクリーンアップします
clean-bench:
	@echo "Removing {{QUANTLIB_DIR}}..."
	@rm -rf bench_data
