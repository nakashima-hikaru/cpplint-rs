# cpplint-rs

A high-performance Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0)

`cpplint-rs` aims to provide **100% functional parity** with the original Google linter while offering significantly faster execution, better accuracy in complex C++ scenarios, and modern CLI features.

## 🚀 Performance

`cpplint-rs` is designed for speed. By leveraging Rust's zero-cost abstractions, multi-threading, and efficient pattern matching, it significantly outperforms the original linter.

### QuantLib Benchmark
_Measured on 2,604 files in the QuantLib codebase._

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 3.230 ± 0.038 | 3.155 | 3.260 | 2.20 ± 0.08 |
| `cpplint-rs` | 1.470 ± 0.051 | 1.364 | 1.518 | 1.00 |

### GoogleTest Benchmark
_Measured on the GoogleTest codebase._

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 269.1 ± 5.3 | 261.3 | 279.2 | 1.94 ± 0.07 |
| `cpplint-rs` | 138.5 ± 4.2 | 132.8 | 147.9 | 1.00 |

## ✨ Highlights

- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.

## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
