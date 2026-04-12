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
    exit 1
fi

# Clone QuantLib and GoogleTest if not already present
if [ ! -d "$BENCH_DIR/QuantLib/.git" ]; then
    echo "Cloning QuantLib..."
    rm -rf "$BENCH_DIR/QuantLib"
    git clone --depth 1 https://github.com/lballabio/QuantLib.git "$BENCH_DIR/QuantLib"
fi

if [ ! -d "$BENCH_DIR/googletest/.git" ]; then
    echo "Cloning GoogleTest..."
    rm -rf "$BENCH_DIR/googletest"
    git clone --depth 1 https://github.com/google/googletest.git "$BENCH_DIR/googletest"
fi

# Build cpplint-rs
echo "Building cpplint-rs..."
cargo build --release --manifest-path "$BASE_DIR/Cargo.toml"
CPPLINT_RS="$BASE_DIR/target/release/cpplint"

# Build/Download cpplint-cpp
if [ ! -f "$BENCH_DIR/cpplint-cpp" ]; then
    echo "Building cpplint-cpp..."
    if [ ! -d "$BENCH_DIR/cpplint-cpp-src/.git" ]; then
        rm -rf "$BENCH_DIR/cpplint-cpp-src"
        git clone --depth 1 https://github.com/matyalatte/cpplint-cpp.git "$BENCH_DIR/cpplint-cpp-src"
    fi

    cd "$BENCH_DIR/cpplint-cpp-src"

    # Verify meson.build exists
    if [ ! -f "meson.build" ]; then
        echo "Error: meson.build not found in cpplint-cpp-src"
        exit 1
    fi

    # Build using meson, explicitly specifying the cpplint target
    rm -rf build
    meson setup build --native-file=presets/release.ini
    meson compile -C build cpplint

    # Find the binary
    if [ -f "build/cpplint" ]; then
        CPP_BIN="build/cpplint"
    else
        CPP_BIN=$(find build -type f -name "cpplint" -executable | head -n 1)
    fi

    if [ -z "$CPP_BIN" ]; then
        echo "Error: Could not find cpplint executable in build directory after explicit build"
        find build -maxdepth 3
        exit 1
    fi

    echo "Found cpplint-cpp binary at: $CPP_BIN"
    cp "$CPP_BIN" "$BENCH_DIR/cpplint-cpp"
    cd "$BASE_DIR"
fi
CPPLINT_CPP="$BENCH_DIR/cpplint-cpp"

# Benchmarking function
run_bench() {
    local target_name=$1
    local target_path=$2

    echo ""
    echo "--- Benchmarking $target_name ---"

    if [ ! -d "$target_path" ]; then
        echo "Error: Target path $target_path does not exist."
        return
    fi

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
