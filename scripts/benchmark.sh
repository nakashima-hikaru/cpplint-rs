#!/bin/bash

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="$BASE_DIR/target/benchmark"
mkdir -p "$BENCH_DIR"

echo "=== Preparing benchmarks ==="

# Check for hyperfine
if ! command -v hyperfine &> /dev/null; then
    echo "hyperfine is not installed. Please install it for accurate benchmarking."
    echo "e.g., brew install hyperfine or cargo install hyperfine"
    exit 1
fi

# Clone QuantLib and GoogleTest if not already present
if [ ! -d "$BENCH_DIR/QuantLib" ]; then
    echo "Cloning QuantLib..."
    git clone --depth 1 https://github.com/lballabio/QuantLib.git "$BENCH_DIR/QuantLib"
fi

if [ ! -d "$BENCH_DIR/googletest" ]; then
    echo "Cloning GoogleTest..."
    git clone --depth 1 https://github.com/google/googletest.git "$BENCH_DIR/googletest"
fi

# Build cpplint-rs
echo "Building cpplint-rs..."
cargo build --release --manifest-path "$BASE_DIR/Cargo.toml"
CPPLINT_RS="$BASE_DIR/target/release/cpplint"

# Build/Download cpplint-cpp
if [ ! -f "$BENCH_DIR/cpplint-cpp" ]; then
    echo "Building cpplint-cpp..."
    if [ ! -d "$BENCH_DIR/cpplint-cpp-src" ]; then
        git clone --depth 1 https://github.com/matyalatte/cpplint-cpp.git "$BENCH_DIR/cpplint-cpp-src"
    fi
    cd "$BENCH_DIR/cpplint-cpp-src"
    mkdir -p build && cd build
    cmake .. -DCMAKE_BUILD_TYPE=Release
    make -j$(nproc 2>/dev/null || sysctl -n hw.ncpu)
    cp cpplint "$BENCH_DIR/cpplint-cpp"
    cd "$BASE_DIR"
fi
CPPLINT_CPP="$BENCH_DIR/cpplint-cpp"

# Benchmarking function
run_bench() {
    local target_name=$1
    local target_path=$2

    echo ""
    echo "--- Benchmarking $target_name ---"

    # We use --ignore-failure because linters will likely find issues in these repos
    # and return non-zero exit codes, which hyperfine would otherwise treat as an error.
    hyperfine --warmup 3 \
        --ignore-failure \
        --export-markdown "$BENCH_DIR/results_${target_name}.md" \
        -n "cpplint-cpp" "$CPPLINT_CPP --recursive $target_path" \
        -n "cpplint-rs" "$CPPLINT_RS --recursive $target_path"

    echo ""
    cat "$BENCH_DIR/results_${target_name}.md"
}

# Run benchmarks
run_bench "QuantLib" "$BENCH_DIR/QuantLib/ql"
run_bench "GoogleTest" "$BENCH_DIR/googletest/googletest"

echo ""
echo "=== Benchmark Complete ==="
echo "Results saved to $BENCH_DIR"
