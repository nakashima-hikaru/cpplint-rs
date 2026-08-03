## 2024-05-20 - Thread-Local Caching for RegEx Optimization
**Learning:** Found a significant bottleneck when reading global caches with an `RwLock` across threads in `regex_utils.rs` and `config.rs`. `RwLock` causes contention when frequently reading in parallel using `rayon`. Moving instance variables (like caching config files or parsing configs) inside `thread_local!` results in memory leaks and unexpected caching behaviors. So thread_local! should only be used for static pure functions like global REGEX_CACHE and CONFIG_FILE_CACHE.
**Action:** Replace `LazyLock<RwLock<T>>` with `thread_local! { static CACHE: RefCell<T> = ... }` for purely global caches heavily accessed inside multi-threaded contexts, especially for hotpath pattern matching and file resolution. This yields a massive speedup (quantlib benchmark time went down from ~590µs to ~315µs).

## 2024-06-25 - Prefer `memchr` over `AhoCorasick` for small predefined patterns
**Learning:** Found that `AhoCorasick` introduces iterator overhead that is relatively slow when searching for small sets of static keywords (like "if" and "else") in hot-path string analysis functions. `memchr2` combined with quick string prefix matching (`starts_with`) significantly outperforms `AhoCorasick` for this type of operation.
**Action:** When optimizing string search for small, predefined patterns on hot paths in Rust, prefer `memchr` combined with a quick byte peek and `starts_with` checks over `AhoCorasick` to minimize iterator overhead.
## 2026-05-06 - Prevent infinite loops in memchr while loop iteration
**Learning:** When using `memchr::memchr` to loop through matches, the returned offset is relative to the provided slice. In a `while let` loop, when modifying `search_start` based on the result, you must break if a match condition is found, otherwise, you might hit an infinite loop if `search_start` is not updated or updated incorrectly within nested structures.
**Action:** When iterating over a slice using `memchr`, properly calculate `abs_pos` and update `search_start` explicitly at the end of the loop iteration to the next start index, and properly label the outer `while` loop if breaking from an inner `for` loop.
## 2026-05-15 - memchr-based substring search optimization for hot paths
**Learning:** In highly-executed word matching functions (like `contains_word` and `contains_word_start`), searching for strings with `str::find` incurs measurable overhead. We discovered that since `memchr::memchr` is already part of our dependency tree (indirectly or via workspace dependencies), we can significantly accelerate these operations.
**Action:** Replace `s[search_start..].find(word)` with a combined approach of `memchr::memchr(word.as_bytes()[0], &s.as_bytes()[search_start..])` followed by a direct byte slice comparison for the remaining characters. This avoids standard library overhead and achieves a ~9% speedup on macro-level benchmarks (`quantlib`).

## 2026-05-31 - Fast path for ASCII string width
**Learning:** Found that `UnicodeWidthChar::width` combined with `nfc()` normalization introduces massive overhead for calculating the display width of strings, even if they only contain standard ASCII characters. Because pure ASCII characters have a width of exactly 1 column and don't change size under NFC normalization, checking if the string is ASCII provides a massive shortcut.
**Action:** Use an early return `if line.is_ascii() { return line.len(); }` before falling back to full unicode normalization and width calculation for line length calculations. This provides an over 100x speedup for pure ASCII lines.

## 2026-06-05 - Binary search for headers check optimization
**Learning:** We saw that looking up strings in large static arrays (e.g. standard header lists and error categories) using `.contains()` iterates over each item resulting in O(N) lookup time. Since the lists are static, they can be pre-sorted and then queried using `binary_search(...).is_ok()` for O(log N) performance. Testing with macro-level benchmarks shows that applying this fast search strategy reduces overhead and significantly improves runtime performance on string validation.
**Action:** For frequent lookups in large static slice arrays, keep the arrays sorted alphabetically and use `.binary_search(...).is_ok()` instead of `.contains()` to improve lookup time from O(N) to O(log N).
## 2026-06-17 - [Fast-Path Text Parsing with `str::contains`]
**Learning:** In C++ linter hot paths (like `parse_access_specifier`), searching multiple keywords across strings is expensive. However, access specifiers always terminate with `:`. `line.contains(':')` avoids all keyword checks using `memchr`-accelerated SIMD instructions for lines that cannot logically match.
**Action:** When a regex or multi-word search pattern requires a specific, single byte (e.g., `:`, `,`, or `{`), check for its presence first using `str::contains(char)` as a fast-path rejection before invoking complex matching logic.

## 2026-07-07 - Avoid format!() allocation in hot loops
**Learning:** Found a performance bottleneck in `check_disallow_macros_at_end` (and potentially others) where `format!("{macro_name}({class_name})")` was dynamically allocating a String on every iteration of a loop checking lines for macro occurrences. Inside hot paths like per-line iteration or token checking, heap allocation adds up significantly.
**Action:** Replace `format!()` with string slice index verification using `.find()` combined with `.starts_with()` checks. This allows searching for dynamically combined substrings entirely without heap allocation.

## 2024-07-10 - Avoiding format! allocations in string manipulation hot paths
**Learning:** `format!()` in Rust introduces significant overhead (argument parsing, formatting traits, and dynamic allocation). In our isolated tests, string manipulation with pre-allocated `String::with_capacity` combined with `push_str` resulted in a >30% speedup.
**Action:** When performing heavily repeated string interpolations in hot loops, explicitly calculate the capacity and use `push_str` instead of relying on the `format!` macro.

## 2024-07-20 - Avoid dynamic string allocation in hot loops using string slicing methods
**Learning:** `format!()` incurs dynamic heap allocation overhead. In `crates/cpplint-core/src/checks/headers.rs`, `format!(".{}", extension.as_str())` was being allocated repeatedly in an iterator loop to check if an include string ends with an extension.
**Action:** Replace `include.ends_with(&format!(".{}", ext))` with `include.strip_suffix(ext).is_some_and(|prefix| prefix.ends_with('.'))`. This leverages zero-allocation string slicing operations and avoids `format!()`, resulting in measurable performance improvements (e.g. ~4% faster on `quantlib` macro benchmark).

## 2024-07-25 - Fast Path Valid UTF-8 and Null Byte check in File Readers
**Learning:** Checking standard library `from_utf8` and slice `contains` over an entire buffer is much faster than checking byte sequences line-by-line. Rust's `std::str::from_utf8` and slice `contains` are highly optimized and use SIMD under the hood. For cases where most files are likely perfectly well-formed, computing this property over the entire file up front creates a fast path that avoids a 100x slower line-by-line fallback.
**Action:** When working on text processing (like file linter and tokenization), try to evaluate conditions on the entire slice early with standard library primitives (e.g. `is_ascii()`, `contains()`, `from_utf8()`) before falling back to manual loops or iterator chaining.
## 2026-07-28 - Avoid format!() in headers check hot paths
**Learning:** `format!()` in Rust introduces significant overhead. In `check_include_line` inside `crates/cpplint-core/src/checks/headers.rs`, `format!("{}.{}", basefilename_relative, ext)` was being repeatedly allocated in a hot loop that iterates over standard header extensions for every `#include` line parsed.
**Action:** Replaced `format!("{}.{}", ...)` inside the loop with a reusable `String` pre-allocated via `String::with_capacity(...)` before the loop. Inside the loop, `string.truncate(...)` and `string.push_str(...)` are used to reuse the buffer without any dynamic memory allocations. Also optimized `path_without_extension` to use string slice lengths instead of `format!(".{ext}")`.
## 2024-05-19 - SIMD Overhead in Line Parsing
**Learning:** In text parsing where typical input lines are short (like typical C++ lines of code), auto-vectorized or manually chunked SIMD (e.g., using `std::simd::u8x32`) can actually be slower than a simple scalar byte-by-byte iteration over a pre-computed lookup table (LUT). The overhead of chunking, processing masks, and handling unaligned tails outweighs the SIMD throughput gains for short strings.
**Action:** When optimizing line-by-line parsing hot paths, favor simple LUT-based scalar loops over manual SIMD chunking unless the expected string lengths are very large (e.g., kilobyte-scale buffers) or the operation is a highly optimized library function like `memchr`.
