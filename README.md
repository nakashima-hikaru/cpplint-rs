# cpplint-rs

A high-performance Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0)

`cpplint-rs` aims to provide **100% functional parity** with the original Google linter while offering significantly faster execution, better accuracy in complex C++ scenarios, and modern CLI features.

## 🚀 Performance

`cpplint-rs` is designed for speed. By leveraging Rust's zero-cost abstractions, multi-threaded execution with `rayon`, and highly efficient pattern matching via Aho-Corasick and SIMD-accelerated scanning, it significantly outperforms the original linter. It further optimizes execution by utilizing bitflags for constant-time state tracking, arena-based memory allocation to minimize heap overhead, and directory-level configuration caching to accelerate massive recursive scans.

### QuantLib Benchmark
_Measured on 2,604 files in the QuantLib codebase._

#### macOS (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 5.533 ± 0.445 | 5.153 | 6.618 | 4.72 ± 0.94 |
| `cpplint-rs` | 1.173 ± 0.214 | 0.878 | 1.478 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 3.107 ± 0.011 | 3.089 | 3.122 | 3.10 ± 0.21 |
| `cpplint-rs` | 1.002 ± 0.067 | 0.889 | 1.115 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 3.689 ± 0.112 | 3.603 | 3.928 | 2.93 ± 0.13 |
| `cpplint-rs` | 1.257 ± 0.042 | 1.175 | 1.299 | 1.00 |

### GoogleTest Benchmark
_Measured on the GoogleTest codebase._

#### macOS (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 639.5 ± 46.2 | 574.6 | 725.1 | 6.17 ± 2.24 |
| `cpplint-rs` | 103.7 ± 36.9 | 62.6 | 190.9 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 249.1 ± 2.3 | 245.3 | 253.0 | 2.49 ± 0.11 |
| `cpplint-rs` | 100.1 ± 4.3 | 92.6 | 107.3 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-cpp` | 312.1 ± 24.1 | 294.5 | 370.1 | 2.90 ± 0.25 |
| `cpplint-rs` | 107.6 ± 3.9 | 102.3 | 117.1 | 1.00 |


## ✨ Highlights

- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.


## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
