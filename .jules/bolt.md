## 2026-04-15 - Extract Regex compilation to LazyLock
**Learning:** Compiling regexes in hotpaths such as string fixers is a noticeable bottleneck for code scanning tools like `cpplint-rs`.
**Action:** Extract inline `Regex::new` to `LazyLock` variables.
## 2026-04-19 - Global locks bottleneck multithreaded caching
**Learning:** Using global locks (`RwLock`) for caches on hot paths (like regex matching) creates severe thread contention when processing in parallel with `rayon`.
**Action:** Use `thread_local!` with `RefCell` to allow lock-free caching per thread and avoid the overhead of synchronization while still minimizing redundant allocations/compilations.
