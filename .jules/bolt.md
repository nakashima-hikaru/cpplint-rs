## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2026-04-15 - Extract Regex compilation to thread_local cache for string operators
**Learning:** In hot loops such as string replacement functions (`add_spaces_around_operator`), dynamically compiling regexes on every call becomes a noticeable overhead. The set of inputs (operators) to these regexes is very small, meaning caching by input string provides a large performance boost. But be careful when introducing caches to avoid global lock contention which can sometimes be slower.
**Action:** Use `thread_local!` with `std::cell::RefCell` and a `FxHashMap` cache for compiled `std::sync::Arc<Regex>` objects to gain performance without incurring multi-threaded locking overheads.

## 2026-04-15 - Optimize string trimming and line indentation checking
**Learning:** For C++ parsing, generic `.trim_start()`/`.trim_end()`/`.trim()` methods search for Unicode whitespace properties which is inefficient compared to explicit ASCII whitespace checking with byte array slices (`.as_bytes()`). Furthermore, the portable SIMD approach can be slower for small sizes (like line indentations) than standard simple byte slice iterators (`.take_while(|&&b| b == b' ')`).
**Action:** Replace `.trim()` functions with explicit ASCII byte slicing loops, and SIMD loops with simple iterator for short strings.
