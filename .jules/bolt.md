## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2026-04-15 - Extract Regex compilation to thread_local cache for string operators
**Learning:** In hot loops such as string replacement functions (`add_spaces_around_operator`), dynamically compiling regexes on every call becomes a noticeable overhead. The set of inputs (operators) to these regexes is very small, meaning caching by input string provides a large performance boost. But be careful when introducing caches to avoid global lock contention which can sometimes be slower.
**Action:** Use `thread_local!` with `std::cell::RefCell` and a `FxHashMap` cache for compiled `std::sync::Arc<Regex>` objects to gain performance without incurring multi-threaded locking overheads.

## 2026-04-15
- **String Allocation in Loops**:  macro calls in tight loops (e.g. ) can be a significant performance bottleneck due to runtime format string parsing and multiple allocations. Replacing them with pre-allocated  and sequential  or  operations yields a measurable ~24% performance improvement in the hot path.

## 2026-04-15
- **String Allocation in Loops**: `format!` macro calls in tight loops (e.g. `cleanse_raw_strings`) can be a significant performance bottleneck due to runtime format string parsing and multiple allocations. Replacing them with pre-allocated `String::with_capacity` and sequential `push_str` or `push` operations yields a measurable ~24% performance improvement in the hot path.
