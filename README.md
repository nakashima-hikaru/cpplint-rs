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
| `cpplint-py` | 112.131 ± 14.279 | 100.513 | 134.477 | 174.25 ± 37.13 |
| `cpplint-cpp` | 4.943 ± 0.158 | 4.822 | 5.348 | 7.68 ± 1.33 |
| `cpplint-rs` | 0.644 ± 0.110 | 0.561 | 0.870 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-py` | 179.872 ± 0.666 | 178.876 | 180.845 | 179.51 ± 12.00 |
| `cpplint-cpp` | 3.107 ± 0.011 | 3.089 | 3.122 | 3.10 ± 0.21 |
| `cpplint-rs` | 1.002 ± 0.067 | 0.889 | 1.115 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-py` | 213.879 ± 7.405 | 209.361 | 230.851 | 120.12 ± 5.09 |
| `cpplint-cpp` | 5.097 ± 0.046 | 5.034 | 5.203 | 2.86 ± 0.07 |
| `cpplint-rs` | 1.781 ± 0.043 | 1.699 | 1.835 | 1.00 |

### GoogleTest Benchmark
_Measured on the GoogleTest codebase._

#### macOS (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-py` | 5942.0 ± 40.0 | 5903.0 | 6028.0 | 96.64 ± 5.77 |
| `cpplint-cpp` | 357.6 ± 4.3 | 352.0 | 364.2 | 5.82 ± 0.35 |
| `cpplint-rs` | 61.5 ± 3.6 | 58.6 | 76.6 | 1.00 |

#### Ubuntu (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-py` | 10753.0 ± 44.0 | 10696.0 | 10818.0 | 104.98 ± 3.63 |
| `cpplint-cpp` | 269.0 ± 4.0 | 263.0 | 275.0 | 2.63 ± 0.10 |
| `cpplint-rs` | 102.0 ± 4.0 | 98.0 | 111.0 | 1.00 |

#### Windows (GitHub Actions)
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `cpplint-py` | 12471.0 ± 132.0 | 12341.0 | 12746.0 | 88.88 ± 4.26 |
| `cpplint-cpp` | 411.1 ± 38.5 | 393.0 | 519.8 | 2.93 ± 0.31 |
| `cpplint-rs` | 140.3 ± 6.6 | 133.8 | 162.1 | 1.00 |


## 🛠️ Autofix Support

`cpplint-rs` goes beyond just reporting issues; it can automatically fix many of them. By using the `--fix` flag, you can resolve various style and formatting violations instantly.

For a detailed list of implemented rules and their autofix status, please refer to [docs/rules.md](docs/rules.md).


## ✨ Highlights

- **Autofix Capability**: Automatically resolve violations for many style and whitespace rules with the `--fix` flag.
- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.


## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
