## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2026-04-17 - Eliminate thread lock overhead in regex caching using `thread_local!`
**Learning:** Global locks like `RwLock` in `regex_utils.rs` cause severe lock contention under multi-threaded execution.
**Action:** Use `thread_local!` with `RefCell` to ensure zero atomic operations during frequent regex lookups in thread pool workers.
