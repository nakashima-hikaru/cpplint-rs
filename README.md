# cpplint-rs

A high-performance Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0)

`cpplint-rs` aims to provide **100% functional parity** with the original Google linter while offering significantly faster execution, better accuracy in complex C++ scenarios, and modern CLI features.

## 🚀 Performance

`cpplint-rs` is designed for speed. By leveraging Rust's zero-cost abstractions, multi-threading, and efficient pattern matching, it significantly outperforms the original linter.

### QuantLib Benchmark
_Measured on 2,604 files in the QuantLib codebase._

#### macOS (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 5.589 ± 0.835 | 4.855 | 7.304 | 4.97 ± 0.87 |
| `cpplint-rs` | 1.125 ± 0.102 | 0.938 | 1.218 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 3.288 ± 0.023 | 3.251 | 3.329 | 2.18 ± 0.07 |
| `cpplint-rs` | 1.509 ± 0.045 | 1.393 | 1.548 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 6.8 ± 1.0 | 5.9 | 11.3 | 1.00 |
| `cpplint-rs` | 6.8 ± 0.9 | 6.1 | 13.0 | 1.01 ± 0.20 |

### GoogleTest Benchmark
_Measured on the GoogleTest codebase._

#### macOS (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 467.4 ± 77.5 | 432.0 | 687.4 | 4.20 ± 0.90 |
| `cpplint-rs` | 111.3 ± 15.0 | 88.0 | 136.7 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 266.0 ± 3.4 | 259.2 | 272.5 | 1.92 ± 0.08 |
| `cpplint-rs` | 138.5 ± 5.4 | 129.9 | 148.5 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 7.9 ± 0.4 | 7.2 | 9.7 | 1.00 |
| `cpplint-rs` | 8.3 ± 0.5 | 7.5 | 11.2 | 1.06 ± 0.08 |

## ✨ Highlights

- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.

## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
