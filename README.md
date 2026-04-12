# cpplint-rs

A high-performance Rust reimplementation of [cpplint 2.0](https://github.com/cpplint/cpplint/tree/2.0.0)

`cpplint-rs` aims to provide **100% functional parity** with the original Google linter while offering significantly faster execution, better accuracy in complex C++ scenarios, and modern CLI features.

## 🚀 Performance

`cpplint-rs` is designed for speed. By leveraging Rust's zero-cost abstractions, multi-threading, and efficient pattern matching, it outperforms even the C++ implementation.

| Benchmark (QuantLib) | cpplint-cpp (C++ rewrite) | cpplint-rs (Rust) | Speedup          |
| :------------------- | :------------------------ | :---------------- | :--------------- |
| **Total Time**       | 5.97 s                    | **0.92 s**        | **~6.5x faster** |
| **Time per File**    | 2.39 ms                   | **0.35 ms**       | **~6.8x faster** |

_Measured on 2,604 files in the QuantLib codebase._

## ✨ Highlights

- **Higher Accuracy**: Handling of complex C++ macros and attributes where the original regex-based linter often fails.
- **Drop-in Match**: Compatible with existing `CPPLINT.cfg` files and command-line arguments.
- **Recursive by Default**: Easily scan entire projects with `--recursive`.
- **Multiple Formats**: Supports `emacs`, `vs7`, `eclipse`, `junit`, and `sed` output formats.
- **Reliable**: Passed extensive validation against GoogleTest and QuantLib codebases.

## 🤝 Acknowledgments

This project was inspired by [cpplint-cpp](https://github.com/matyalatte/cpplint-cpp), which demonstrated the potential for a high-performance compiled alternative to the original Python script.
