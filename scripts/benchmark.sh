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
if [ -f "${CPPLINT_RS}.exe" ]; then
    CPPLINT_RS="${CPPLINT_RS}.exe"
fi

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

    # Build using meson
    rm -rf build
    meson setup build --native-file=presets/release.ini
    meson compile -C build

    # Find the binary (prioritize .exe for Windows MSYS compat)
    if [ -f "build/cpplint-cpp.exe" ]; then
        CPP_BIN="build/cpplint-cpp.exe"
    elif [ -f "build/cpplint.exe" ]; then
        CPP_BIN="build/cpplint.exe"
    elif [ -f "build/src/cpplint-cpp.exe" ]; then
        CPP_BIN="build/src/cpplint-cpp.exe"
    elif [ -f "build/cpplint-cpp" ]; then
        CPP_BIN="build/cpplint-cpp"
    elif [ -f "build/cpplint" ]; then
        CPP_BIN="build/cpplint"
    elif [ -f "build/src/cpplint-cpp" ]; then
        CPP_BIN="build/src/cpplint-cpp"
    else
        CPP_BIN=$(find build -type f \( -name "cpplint-cpp.exe" -o -name "cpplint.exe" -o -name "cpplint-cpp" -o -name "cpplint" \) -executable | grep -v "subprojects" | head -n 1)
    fi

    if [ -z "$CPP_BIN" ]; then
        echo "Error: Could not find cpplint executable in build directory after explicit build"
        find build -maxdepth 3
        exit 1
    fi

    echo "Found cpplint-cpp binary at: $CPP_BIN"
    if [[ "$CPP_BIN" == *.exe ]]; then
        cp "$CPP_BIN" "$BENCH_DIR/cpplint-cpp.exe"
    else
        cp "$CPP_BIN" "$BENCH_DIR/cpplint-cpp"
    fi
    cd "$BASE_DIR"
fi
CPPLINT_CPP="$BENCH_DIR/cpplint-cpp"
if [ -f "${CPPLINT_CPP}.exe" ]; then
    CPPLINT_CPP="${CPPLINT_CPP}.exe"
fi

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

    # If running under MSYS/Cygwin, convert target_path and binaries to Windows format
    local run_path="$target_path"
    local cpp_bin="$CPPLINT_CPP"
    local rs_bin="$CPPLINT_RS"

    if command -v cygpath &> /dev/null; then
        run_path=$(cygpath -w "$target_path")
        cpp_bin=$(cygpath -w "$CPPLINT_CPP")
        rs_bin=$(cygpath -w "$CPPLINT_RS")
    fi

    echo "Debugging paths before hyperfine:"
    echo "cpp_bin = $cpp_bin"
    echo "rs_bin  = $rs_bin"
    echo "run_path = $run_path"

    # We use --ignore-failure because linters will likely find issues in these repos
    # and return non-zero exit codes, which hyperfine would otherwise treat as an error.
    # Note: we do not add inner quotes around variables here because cmd.exe handles quotes poorly.
    # GitHub Action paths (/d/a/...) generally do not contain spaces.
    hyperfine --warmup 3 \
        --ignore-failure \
        --export-markdown "$BENCH_DIR/results_${target_name}.md" \
        -n "cpplint-cpp" "$cpp_bin --recursive $run_path" \
        -n "cpplint-rs" "$rs_bin --recursive $run_path"

    echo ""
    cat "$BENCH_DIR/results_${target_name}.md"
}

# Run benchmarks
run_bench "QuantLib" "$BENCH_DIR/QuantLib/ql"
run_bench "GoogleTest" "$BENCH_DIR/googletest/googletest"

echo ""
echo "=== Benchmark Complete ==="
echo "Results saved to $BENCH_DIR"
