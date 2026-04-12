use crate::errors::Result;
use globset::{GlobBuilder, GlobMatcher};

pub struct GlobPattern {
    matcher: GlobMatcher,
}

impl GlobPattern {
    pub fn new(pattern: &str, match_with_parent: bool) -> Result<Self> {
        let mut final_pattern = pattern.to_string();

        // Normalize slashes
        final_pattern = final_pattern.replace('\\', "/");

        if match_with_parent {
            if final_pattern.ends_with('/') {
                final_pattern.push_str("**");
            } else if !final_pattern.ends_with("**") {
                final_pattern = format!("{{{},{}/**}}", final_pattern, final_pattern);
            }
        }

        let glob = GlobBuilder::new(&final_pattern)
            .literal_separator(true) // * doesn't match /
            .backslash_escape(true)
            .build()?;

        Ok(GlobPattern {
            matcher: glob.compile_matcher(),
        })
    }

    pub fn is_match(&self, path: &str) -> bool {
        // Normalize path for matching (C++ version handled both separators)
        let normalized_path = path.replace('\\', "/");
        self.matcher.is_match(&normalized_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct GlobCase {
        pattern: &'static str,
        path: &'static str,
        expected: bool,
        expected_parent: bool,
    }

    #[test]
    fn test_glob_cases() {
        let cases = vec![
            // literal
            GlobCase {
                pattern: "/foo/bar.h",
                path: "/foo/bar.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/bar.h",
                path: "/foo/bar-h",
                expected: false,
                expected_parent: false,
            },
            // any characters
            GlobCase {
                pattern: "/foo/*h",
                path: "/foo/bar-h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/bar/*h",
                path: "/foo/bar-h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "/foo/bar/*h",
                path: "/foo/bar/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/*/test.h",
                path: "/foo/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/*/test.h",
                path: "/foo/bar/test.h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "foo/*",
                path: "foo/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "foo/*",
                path: "foo/bar/test.h",
                expected: false,
                expected_parent: true,
            },
            // recursive
            GlobCase {
                pattern: "/**/test.h",
                path: "/foo/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/**/test.h",
                path: "/foo/bar/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/**bar/test.h",
                path: "/foo/bar/test.h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "**/test.h",
                path: "/foo/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "**/test.h",
                path: "/foo/bar/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "**/test.h",
                path: "test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "**bar/test.h",
                path: "/foo/bar/test.h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "foo/**",
                path: "foo/test.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "foo/**",
                path: "foo/bar/test.h",
                expected: true,
                expected_parent: true,
            },
            // any single character
            GlobCase {
                pattern: "/foo/bar?h",
                path: "/foo/bar.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/bar?h",
                path: "/foo/bar..h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "/foo/bar?h",
                path: "/foo/bar/h",
                expected: false,
                expected_parent: false,
            },
            // list
            GlobCase {
                pattern: "/foo/[abc].h",
                path: "/foo/b.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/[abc].h",
                path: "/foo/d.h",
                expected: false,
                expected_parent: false,
            },
            // negative list
            GlobCase {
                pattern: "/foo/[!abc].h",
                path: "/foo/b.h",
                expected: false,
                expected_parent: false,
            },
            GlobCase {
                pattern: "/foo/[!abc].h",
                path: "/foo/d.h",
                expected: true,
                expected_parent: true,
            },
            // range
            GlobCase {
                pattern: "/foo/[a-c].h",
                path: "/foo/b.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/[a-c].h",
                path: "/foo/d.h",
                expected: false,
                expected_parent: false,
            },
            // compare with parent matching
            GlobCase {
                pattern: "/foo/bar",
                path: "/foo/bar/baz",
                expected: false,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/*",
                path: "/foo/bar/baz",
                expected: false,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/",
                path: "/foo/bar/baz",
                expected: false,
                expected_parent: true,
            },
            GlobCase {
                pattern: "/foo/**/test",
                path: "/foo/bar/baz/test/a.cpp",
                expected: false,
                expected_parent: true,
            },
            // windows paths
            GlobCase {
                pattern: "C:/foo/bar.h",
                path: "C:\\foo\\bar.h",
                expected: true,
                expected_parent: true,
            },
            GlobCase {
                pattern: "C:/foo/bar",
                path: "C:\\foo\\bar\\baz",
                expected: false,
                expected_parent: true,
            },
            GlobCase {
                pattern: "C:\\foo\\bar",
                path: "C:/foo/bar/baz",
                expected: false,
                expected_parent: true,
            },
        ];

        for (i, c) in cases.iter().enumerate() {
            // Test normal match
            let gp = GlobPattern::new(c.pattern, false)
                .unwrap_or_else(|_| panic!("Case {} pattern failed", i));
            assert_eq!(
                gp.is_match(c.path),
                c.expected,
                "Case {} Normal (pattern: {}, path: {})",
                i,
                c.pattern,
                c.path
            );

            // Test parent match
            let gp_p = GlobPattern::new(c.pattern, true)
                .unwrap_or_else(|_| panic!("Case {} parent pattern failed", i));
            assert_eq!(
                gp_p.is_match(c.path),
                c.expected_parent,
                "Case {} Parent (pattern: {}, path: {})",
                i,
                c.pattern,
                c.path
            );
        }
    }
}
