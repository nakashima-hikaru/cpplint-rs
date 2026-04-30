
QUANTLIB_DIR := "bench_data/QuantLib"
QUANTLIB_URL := "https://github.com/lballabio/QuantLib.git"

bench-quantlib: clone-quantlib
	@echo "Running Criterion benchmark for QuantLib..."
	cargo bench -p cpplint-core --bench quantlib

clone-quantlib:
	@if [ ! -d "{{QUANTLIB_DIR}}" ]; then \
		echo "Cloning QuantLib into {{QUANTLIB_DIR}}..."; \
		mkdir -p bench_data; \
		git clone --depth 1 {{QUANTLIB_URL}} {{QUANTLIB_DIR}} --quiet; \
	fi

build-release:
	@echo "Building cpplint-rs in release mode..."
	@cargo build --release --quiet

measure-quantlib: clone-quantlib build-release
	@echo "Measuring memory usage and time for QuantLib..."
	@(/usr/bin/time -l ./target/release/cpplint --recursive {{QUANTLIB_DIR}} > /dev/null) 2>&1 | tail -n 20

clean-bench:
	@echo "Removing {{QUANTLIB_DIR}}..."
	@rm -rf bench_data

test-upstream-cpplint:
	@echo "Running upstream cpplint_unittest.py..."
	@cd tests/upstream_cpplint && python3 -m pytest cpplint_unittest.py
