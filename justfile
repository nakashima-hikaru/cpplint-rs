
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

# ベンチマーク用のデータをクリーンアップします
clean-bench:
	@echo "Removing {{QUANTLIB_DIR}}..."
	@rm -rf bench_data
