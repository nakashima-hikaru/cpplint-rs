# cpplint-rs

A high-performance Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0)

`cpplint-rs` aims to provide **100% functional parity** with the original Google linter while offering significantly faster execution, better accuracy in complex C++ scenarios, and modern CLI features.

## 🚀 Performance

`cpplint-rs` is designed for speed. By leveraging Rust's zero-cost abstractions, multi-threading, and efficient pattern matching, it significantly outperforms the original linter.

### QuantLib Benchmark
_Measured on 2,604 files in the QuantLib codebase._

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 3.118 ± 0.013 | 3.099 | 3.143 | 2.02 ± 0.11 |
| `cpplint-rs` | 1.544 ± 0.081 | 1.422 | 1.619 | 1.00 |

### GoogleTest Benchmark
_Measured on the GoogleTest codebase._

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 252.0 ± 3.9 | 247.1 | 259.0 | 1.75 ± 0.07 |
| `cpplint-rs` | 144.0 ± 5.3 | 134.2 | 152.6 | 1.00 |

## ✨ Highlights

- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.

## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
