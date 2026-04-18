## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2024-05-18 - Replacing AhoCorasick with memchr on hot paths
**Learning:** For small, predefined sets of keywords on hot paths, AhoCorasick introduces more overhead than using  combined with a quick byte peek and  checks.
**Action:** Always prefer  combined with  when searching for a small number of keywords on extremely hot string processing paths.
## 2024-05-18 - Replacing AhoCorasick with memchr on hot paths
**Learning:** For small, predefined sets of keywords on hot paths, AhoCorasick introduces more overhead than using `memchr` combined with a quick byte peek and `starts_with` checks.
**Action:** Always prefer `memchr` combined with `.starts_with()` when searching for a small number of keywords on extremely hot string processing paths.
