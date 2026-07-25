# cpplint-rs

A fast Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0).

`cpplint-rs` aims for functional parity with the original Google linter while running much faster, handling tricky C++ code more accurately, and providing a more modern CLI.

## 🚀 Quick start

Install the CLI from this repository:

```sh
cargo install --path crates/cpplint-cli
```

Run it on a project:

```sh
cpplint --recursive path/to/project
```

Apply supported autofixes:

```sh
cpplint --fix --recursive path/to/project
```

For the full list of implemented rules and current autofix coverage, see [docs/rules.md](docs/rules.md).

## ✨ Why cpplint-rs

- **Fast**: Designed to be dramatically faster than the original Python implementation.
- **Autofix support**: Fix many style, formatting, readability, and runtime issues with `--fix`.
- **Better C++ handling**: Deals more reliably with complex macros, attributes, and modern C++ syntax.
- **Drop-in compatibility**: Works with existing `CPPLINT.cfg` files and familiar command-line flags.
- **Recursive scanning**: Lint entire projects with `--recursive`.
- **Multiple output formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed`.

## 📈 Performance

`cpplint-rs` is built for speed. It uses Rust, `rayon` for parallel execution, Aho-Corasick for fast pattern matching, and SIMD-friendly scanning to outperform the original linter by a wide margin. It also reduces overhead with compact state tracking, arena-based allocation, and directory-level configuration caching for large recursive scans.

### QuantLib Benchmark

_Measured on 2,604 files in the QuantLib codebase._

#### macOS (GitHub Actions)

| Command       |         Mean [s] | Min [s] | Max [s] |       Relative |
| :------------ | ---------------: | ------: | ------: | -------------: |
| `cpplint-py`  | 112.131 ± 14.279 | 100.513 | 134.477 | 273.49 ± 35.86 |
| `cpplint-cpp` |    4.943 ± 0.158 |   4.822 |   5.348 |   12.06 ± 0.54 |
| `cpplint-rs`  |    0.410 ± 0.013 |   0.391 |   0.430 |           1.00 |

#### Ubuntu (GitHub Actions)

| Command       |        Mean [s] | Min [s] | Max [s] |      Relative |
| :------------ | --------------: | ------: | ------: | ------------: |
| `cpplint-py`  | 179.872 ± 0.666 | 178.876 | 180.845 | 291.95 ± 13.41 |
| `cpplint-cpp` |   3.107 ± 0.011 |   3.089 |   3.122 |    5.04 ± 0.23 |
| `cpplint-rs`  |    0.6161 ± 0.0282 |   0.5943 |   0.6946 |          1.00 |

#### Windows (GitHub Actions)

| Command       |        Mean [s] | Min [s] | Max [s] |      Relative |
| :------------ | --------------: | ------: | ------: | ------------: |
| `cpplint-py`  | 213.879 ± 7.405 | 209.361 | 230.851 | 203.69 ± 12.15 |
| `cpplint-cpp` |   5.097 ± 0.046 |   5.034 |   5.203 |    4.85 ± 0.24 |
| `cpplint-rs`  |   1.050 ± 0.051 |   0.990 |   1.151 |          1.00 |

### GoogleTest Benchmark

_Measured on the GoogleTest codebase._

#### macOS (GitHub Actions)

| Command       |     Mean [ms] | Min [ms] | Max [ms] |      Relative |
| :------------ | ------------: | -------: | -------: | ------------: |
| `cpplint-py`  | 5942.0 ± 40.0 |   5903.0 |   6028.0 | 97.25 ± 23.72 |
| `cpplint-cpp` |   357.6 ± 4.3 |    352.0 |    364.2 |    5.85 ± 1.43 |
| `cpplint-rs`  |    61.1 ± 14.9 |     41.2 |     97.2 |          1.00 |

#### Ubuntu (GitHub Actions)

| Command       |      Mean [ms] | Min [ms] | Max [ms] |      Relative |
| :------------ | -------------: | -------: | -------: | ------------: |
| `cpplint-py`  | 10753.0 ± 44.0 |  10696.0 |  10818.0 | 148.93 ± 3.56 |
| `cpplint-cpp` |    269.0 ± 4.0 |    263.0 |    275.0 |    3.73 ± 0.10 |
| `cpplint-rs`  |    72.2 ± 1.7 |     69.3 |     76.5 |          1.00 |

#### Windows (GitHub Actions)

| Command       |       Mean [ms] | Min [ms] | Max [ms] |      Relative |
| :------------ | --------------: | -------: | -------: | ------------: |
| `cpplint-py`  | 12471.0 ± 132.0 |  12341.0 |  12746.0 | 135.85 ± 2.90 |
| `cpplint-cpp` |    411.1 ± 38.5 |    393.0 |    519.8 |    4.48 ± 0.43 |
| `cpplint-rs`  |     91.8 ± 1.7 |     88.7 |     97.8 |          1.00 |

## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which showed that a high-performance compiled alternative to the original Python script was both practical and valuable.
