## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2026-04-15 - Extract Regex compilation to thread_local cache for string operators
**Learning:** In hot loops such as string replacement functions (`add_spaces_around_operator`), dynamically compiling regexes on every call becomes a noticeable overhead. The set of inputs (operators) to these regexes is very small, meaning caching by input string provides a large performance boost. But be careful when introducing caches to avoid global lock contention which can sometimes be slower.
**Action:** Use `thread_local!` with `std::cell::RefCell` and a `FxHashMap` cache for compiled `std::sync::Arc<Regex>` objects to gain performance without incurring multi-threaded locking overheads.
2024-05-23: When extracting string prefixes or slices, avoid character iteration loops that 'collect::<String>()', as this triggers redundant allocations. Use slice indices or functions like `char_indices()` combined with slice creation `&str[..]` to avoid heap allocations. Replaced `String::clone()` in hot loops with `std::borrow::Cow` to lazily clone, eliminating allocations on unmodified inputs.

## 2026-04-15
* Optimizing AhoCorasick for simple needles in hot path (`runtime.rs`)
  Replaced AhoCorasick search logic with native `memchr` plus simple byte lookup and iteration. This brought down micro-benchmark timing significantly (~60ns to ~22ns in matched case and 58ns to 25ns in unmatched case). While it doesn't dramatically shift the overall `quantlib` macro benchmark, replacing heavier DFAs with manual `memchr` combined with bounds-checked peeking is a strong pattern for simple, predictable match sets in tight per-line parsing loops.
