## 2026-04-16
When extracting string prefixes or slices, avoid character iteration loops that 'collect::<String>()', as this triggers redundant allocations. Use slice indices or functions like `char_indices()` combined with slice creation `&str[..]` to avoid heap allocations. Replaced `String::clone()` in hot loops with `std::borrow::Cow` to lazily clone, eliminating allocations on unmodified inputs.

## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.

## 2026-04-15 - Extract Regex compilation to thread_local cache for string operators
**Learning:** In hot loops such as string replacement functions (`add_spaces_around_operator`), dynamically compiling regexes on every call becomes a noticeable overhead. The set of inputs (operators) to these regexes is very small, meaning caching by input string provides a large performance boost. But be careful when introducing caches to avoid global lock contention which can sometimes be slower.
**Action:** Use `thread_local!` with `std::cell::RefCell` and a `FxHashMap` cache for compiled `std::sync::Arc<Regex>` objects to gain performance without incurring multi-threaded locking overheads.
## 2026-04-15 - Optimizing AhoCorasick for simple needles in hot path (`runtime.rs`)
**Learning:** Replaced AhoCorasick search logic with native `memchr` plus simple byte lookup and iteration. This brought down micro-benchmark timing significantly (~60ns to ~22ns in matched case and 58ns to 25ns in unmatched case). While it doesn't dramatically shift the overall `quantlib` macro benchmark, replacing heavier DFAs with manual `memchr` combined with bounds-checked peeking is a strong pattern for simple, predictable match sets in tight per-line parsing loops.

## 2026-04-15 - Optimize string trimming and line indentation checking
**Learning:** For C++ parsing, generic `.trim_start()`/`.trim_end()`/`.trim()` methods search for Unicode whitespace properties which is inefficient compared to explicit ASCII whitespace checking with byte array slices (`.as_bytes()`). Furthermore, the portable SIMD approach can be slower for small sizes (like line indentations) than standard simple byte slice iterators (`.take_while(|&&b| b == b' ')`).
**Action:** Replace `.trim()` functions with explicit ASCII byte slicing loops, and SIMD loops with simple iterator for short strings.

## 2026-04-15 - String Allocation in Loops
**Learning:** `format!` macro calls in tight loops (e.g. `cleanse_raw_strings`) can be a significant performance bottleneck due to runtime format string parsing and multiple allocations.
**Action:** Replacing them with pre-allocated `String::with_capacity` and sequential `push_str` or `push` operations yields a measurable ~24% performance improvement in the hot path.

## 2026-04-15 - Regex Compilation Caching
**Learning:** When compiling identical regex strings multiple times across config parsing boundaries (or similar contexts), cache the compiled `Regex` (or its parsing `Error`) inside an `Arc` to avoid the overhead of repeatedly calling `Regex::new`. Caching the `Result<Arc<Regex>, regex::Error>` allows preserving validation errors while still avoiding duplicate compilation overhead for repeatedly seen valid *and* invalid strings.
