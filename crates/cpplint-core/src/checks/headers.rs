use crate::c_headers;
use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use crate::options::IncludeOrder;
use crate::state::{IncludeKind, IncludeState};
use aho_corasick::AhoCorasick;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static INCLUDE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^\s*#\s*include\s*([<"])([^>"]+)[>"]"#).unwrap());

#[derive(Clone, Copy)]
enum IwyuKind {
    Word,
    FuncOrTempl,
    StdTempl,
    Templ,
    Func,
}

struct IwyuCheck {
    token: &'static str,
    header: &'static str,
    kind: IwyuKind,
}

const IWYU_CHECKS: &[IwyuCheck] = &[
    IwyuCheck {
        token: "string",
        header: "string",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "cin",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "cout",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "cerr",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "clog",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "wcin",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "wcout",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "wcerr",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "wclog",
        header: "iostream",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "FILE",
        header: "cstdio",
        kind: IwyuKind::Word,
    },
    IwyuCheck {
        token: "fpos_t",
        header: "cstdio",
        kind: IwyuKind::Word,
    },
    // Algorithm
    IwyuCheck {
        token: "copy",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "max",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "min",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "min_element",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "sort",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "transform",
        header: "algorithm",
        kind: IwyuKind::FuncOrTempl,
    },
    // Utility
    IwyuCheck {
        token: "forward",
        header: "utility",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "make_pair",
        header: "utility",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "move",
        header: "utility",
        kind: IwyuKind::FuncOrTempl,
    },
    IwyuCheck {
        token: "swap",
        header: "utility",
        kind: IwyuKind::FuncOrTempl,
    },
    // Map
    IwyuCheck {
        token: "map",
        header: "map",
        kind: IwyuKind::StdTempl,
    },
    // Templates
    IwyuCheck {
        token: "unary_function",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "binary_function",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "plus",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "minus",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "multiplies",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "divides",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "modulus",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "negate",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "equal_to",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "not_equal_to",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "greater",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "less",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "greater_equal",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "less_equal",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "logical_and",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "logical_or",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "logical_not",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "unary_negate",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "not1",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "binary_negate",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "not2",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "bind1st",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "bind2nd",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "pointer_to_unary_function",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "pointer_to_binary_function",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "ptr_fun",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun1_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun1_ref_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun_ref_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "const_mem_fun_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "const_mem_fun1_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "const_mem_fun_ref_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "const_mem_fun1_ref_t",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "mem_fun_ref",
        header: "functional",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "list",
        header: "list",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "numeric_limits",
        header: "limits",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "multimap",
        header: "map",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "allocator",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "make_shared",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "make_unique",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "shared_ptr",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "unique_ptr",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "weak_ptr",
        header: "memory",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "set",
        header: "set",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "char_traits",
        header: "string",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "tuple",
        header: "tuple",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "pair",
        header: "utility",
        kind: IwyuKind::Templ,
    },
    IwyuCheck {
        token: "vector",
        header: "vector",
        kind: IwyuKind::Templ,
    },
    // cstdio functions
    IwyuCheck {
        token: "fgets",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fclose",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "clearerr",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "feof",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "ferror",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fflush",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fgetpos",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fread",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fgetc",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fputc",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fputs",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fopen",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "freopen",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fseek",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fsetpos",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "ftell",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "getc",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "putc",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "putchar",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "perror",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "printf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "puts",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "scanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "setbuf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "setvbuf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "snprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "sprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "sscanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "tmpnam",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "ungetc",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vfprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vfscanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vsnprintf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vscanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "vsscanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fwrite",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
    IwyuCheck {
        token: "fscanf",
        header: "cstdio",
        kind: IwyuKind::Func,
    },
];

static IWYU_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let patterns: Vec<&str> = IWYU_CHECKS.iter().map(|c| c.token).collect();
    aho_corasick::AhoCorasickBuilder::new()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(patterns)
        .unwrap()
});

static SPECIAL_INCLUDE_NEEDLES: [&str; 3] = ["lua.h", "lauxlib.h", "lualib.h"];
static SPECIAL_INCLUDE_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(SPECIAL_INCLUDE_NEEDLES).unwrap());

static NOLINT_HEADER_GUARD_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"//\s*NOLINT\(build/header_guard\)").unwrap());
static PRAGMA_ONCE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^\s*#pragma\s+once\b").unwrap());

pub fn check_header_guard(linter: &mut FileLinter, clean_lines: &CleansedLines) {
    let extension = Path::new(linter.filename())
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if !linter.options().header_extensions().contains(extension) {
        return;
    }

    let raw_lines = &clean_lines.lines_without_raw_strings;

    // Respect the documented file-level suppression for synthetic guard errors.
    for line in raw_lines {
        if NOLINT_HEADER_GUARD_RE.is_match(line) {
            return;
        }
    }

    // 1. Check for #pragma once
    for line in raw_lines {
        if PRAGMA_ONCE_RE.is_match(line) {
            return;
        }
    }

    let expected_guard = generate_guard(&linter.header_guard_path());

    // 3. Search for #ifndef and #define
    let mut ifndef = None;
    let mut define = None;
    let mut endif = None;
    let mut endif_line = None;

    for (i, line) in raw_lines.iter().enumerate() {
        if let Some(stripped) = line.strip_prefix("#ifndef ") {
            if ifndef.is_none() {
                ifndef = Some((i, stripped.trim().to_string()));
            }
        } else if let Some(stripped) = line.strip_prefix("#define ") {
            if define.is_none() {
                define = Some(stripped.trim().to_string());
            }
        } else if line.starts_with("#endif") {
            endif = Some(i);
            endif_line = Some(line.trim().to_string());
        }
    }

    if let (Some((line_idx, guard)), Some(d_guard)) = (ifndef, define)
        && guard == d_guard {
            if guard != expected_guard {
                linter.error(
                    line_idx,
                    "build/header_guard",
                    5,
                    &format!(
                        "#ifndef header guard has wrong style, please use: {}",
                        expected_guard
                    ),
                );
            }

            let endif_idx = endif.unwrap_or(raw_lines.len().saturating_sub(1));
            let endif_line = endif_line.unwrap_or_default();
            let expected_slash = format!("#endif  // {}", expected_guard);
            let expected_block = format!("#endif  /* {} */", expected_guard);
            let expected_slash_legacy = format!("#endif  // {}_", expected_guard);
            let expected_block_legacy = format!("#endif  /* {}_ */", expected_guard);

            if endif_line == expected_slash || endif_line == expected_block {
                return;
            }

            if endif_line == expected_slash_legacy {
                linter.error(
                    endif_idx,
                    "build/header_guard",
                    0,
                    &format!(r#"#endif line should be "{}""#, expected_slash),
                );
                return;
            }

            if endif_line == expected_block_legacy {
                linter.error(
                    endif_idx,
                    "build/header_guard",
                    0,
                    &format!(r#"#endif line should be "{}""#, expected_block),
                );
                return;
            }

            linter.error(
                endif_idx,
                "build/header_guard",
                5,
                &format!(r#"#endif line should be "{}""#, expected_slash),
            );
            return;
        }

    linter.error_display_line(
        0,
        "build/header_guard",
        5,
        &format!(
            "No #ifndef header guard found, suggested CPP variable is: {}",
            expected_guard
        ),
    );
}

pub fn check_includes(linter: &mut FileLinter, clean_lines: &CleansedLines) {
    let mut include_state = IncludeState::new();
    let all_extensions = linter.options().all_extensions();
    let header_extensions = linter.options().header_extensions();
    let non_header_extensions: Vec<String> = all_extensions
        .difference(&header_extensions)
        .cloned()
        .collect();
    let file_from_repo = linter.relative_from_repository();
    let file_from_repo_dir = file_from_repo.parent().unwrap_or_else(|| Path::new(""));
    let file_from_repo_str = file_from_repo.to_string_lossy().replace('\\', "/");
    let basefilename_relative = file_from_repo_str
        .strip_suffix(&format!(
            ".{}",
            file_from_repo
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
        ))
        .unwrap_or(&file_from_repo_str)
        .to_string();

    for (linenum, line) in clean_lines.lines_without_raw_strings.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('#') && !INCLUDE_RE.is_match(trimmed) {
            if let Some(directive) = preprocessor_directive(trimmed) {
                include_state.reset_section(directive);
            }
            continue;
        }

        let Some(captures) = INCLUDE_RE.captures(trimmed) else {
            continue;
        };

        let delim = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let include = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let used_angle_brackets = delim == "<";
        let kind = classify_include(
            &file_from_repo,
            Path::new(include),
            used_angle_brackets,
            linter.options().include_order,
        );
        if delim == "\""
            && !include.contains('/')
            && header_extensions.contains(
                Path::new(include)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or(""),
            )
            && !is_special_include_name(include)
            && !matches!(
                kind,
                IncludeKind::LikelyMyHeader | IncludeKind::PossibleMyHeader
            )
        {
            linter.error(
                linenum,
                "build/include_subdir",
                4,
                "Include the directory when naming header files",
            );
        }

        if matches!(include, "cfenv" | "fenv.h" | "ratio") {
            linter.error(
                linenum,
                "build/c++11",
                5,
                &format!("<{}> is an unapproved C++11 header.", include),
            );
        }

        if include == "filesystem" {
            linter.error(
                linenum,
                "build/c++17",
                5,
                "<filesystem> is an unapproved C++17 header.",
            );
        }

        let has_nolint = clean_lines.raw_lines[linenum].contains("NOLINT");

        if let Some(first_line) = include_state.find_header(include) {
            if has_nolint {
                include_state
                    .last_include_list_mut()
                    .push((include.to_string(), linenum));
                continue;
            }
            linter.error(
                linenum,
                "build/include",
                4,
                &format!(
                    r#""{}" already included at {}:{}"#,
                    include,
                    linter.filename(),
                    first_line + 1
                ),
            );
            continue;
        }

        let includes_non_header_from_other_package =
            non_header_extensions.iter().find(|extension| {
                include.ends_with(&format!(".{}", extension.as_str()))
                    && file_from_repo_dir
                        != Path::new(include).parent().unwrap_or_else(|| Path::new(""))
            });
        if let Some(extension) = includes_non_header_from_other_package {
            linter.error(
                linenum,
                "build/include",
                4,
                &format!("Do not include .{} files from other packages", extension),
            );
            continue;
        }

        let third_src_header = header_extensions.iter().any(|ext| {
            let headername = format!("{}.{}", basefilename_relative, ext);
            headername.contains(include) || include.contains(&headername)
        });
        if third_src_header || !is_special_include_name(include) {
            include_state
                .last_include_list_mut()
                .push((include.to_string(), linenum));
            if let Some(message) = include_state.check_next_include_order(kind) {
                let basename = Path::new(linter.filename())
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("");
                linter.error(
                    linenum,
                    "build/include_order",
                    4,
                    &format!(
                        "{}. Should be: {}.h, c system, c++ system, other.",
                        message, basename
                    ),
                );
            }

            let canonical_include = include_state.canonicalize_alphabetical_order(include);
            let previous_line_is_include =
                linenum > 0 && INCLUDE_RE.is_match(clean_lines.elided[linenum - 1].trim());
            if !include_state.is_in_alphabetical_order(previous_line_is_include, &canonical_include)
            {
                linter.error(
                    linenum,
                    "build/include_alpha",
                    4,
                    &format!(r#"Include "{}" not in alphabetical order"#, include),
                );
            }
            include_state.set_last_header(&canonical_include);
        }
    }

    check_include_what_you_use(linter, clean_lines, &include_state);
    check_header_file_included(linter, &include_state);
}

fn classify_include(
    path_from_repo: &Path,
    include: &Path,
    used_angle_brackets: bool,
    include_order: IncludeOrder,
) -> IncludeKind {
    let include_str = include.to_string_lossy().replace('\\', "/");
    let is_cpp_header = c_headers::CPP_HEADERS.contains(&include_str.as_str());
    let include_ext = include
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext))
        .unwrap_or_default();
    let is_system =
        used_angle_brackets && !matches!(include_ext.as_str(), ".hh" | ".hpp" | ".hxx" | ".h++");
    let is_std_c_header = include_order == IncludeOrder::Default
        || c_headers::C_HEADERS.contains(&include_str.as_str());

    if is_system {
        return if is_cpp_header {
            IncludeKind::CppSystem
        } else if is_std_c_header {
            IncludeKind::CSystem
        } else {
            IncludeKind::OtherSystem
        };
    }

    let target_file = drop_common_suffixes(path_from_repo);
    let target_dir = target_file.parent().unwrap_or_else(|| Path::new(""));
    let target_base = target_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let include_file = drop_common_suffixes(include);
    let include_dir = include_file.parent().unwrap_or_else(|| Path::new(""));
    let include_base = include_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let target_dir_pub = normalize_path(&target_dir.join("../public"));
    if target_base == include_base
        && (normalize_path(include_dir) == normalize_path(target_dir)
            || normalize_path(include_dir) == target_dir_pub)
    {
        return IncludeKind::LikelyMyHeader;
    }

    if first_component(target_base) == first_component(include_base) {
        return IncludeKind::PossibleMyHeader;
    }

    IncludeKind::OtherHeader
}

fn preprocessor_directive(trimmed: &str) -> Option<&str> {
    let directive = trimmed.strip_prefix('#')?.trim_start();
    ["if", "ifdef", "ifndef", "else", "elif", "endif"]
        .into_iter()
        .find(|candidate| directive.starts_with(candidate))
}

static SPECIAL_HEADER_NAME_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^[^/]*[A-Z][^/]*\.h$"#).unwrap());

fn is_special_include_name(include: &str) -> bool {
    if SPECIAL_INCLUDE_AC.is_match(include) {
        return true;
    }
    SPECIAL_HEADER_NAME_RE.is_match(include)
}

fn drop_common_suffixes(path: &Path) -> PathBuf {
    let value = path.to_string_lossy().replace('\\', "/");
    for suffix in [
        "-inl.h", ".h", ".hh", ".hpp", ".hxx", ".h++", ".c", ".cc", ".cpp", ".cxx",
    ] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            return PathBuf::from(stripped);
        }
    }
    PathBuf::from(value)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn first_component(value: &str) -> &str {
    value.split(['-', '_', '.']).next().unwrap_or(value)
}

fn check_include_what_you_use(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    include_state: &IncludeState,
) {
    let mut required: BTreeMap<&str, (usize, String)> = BTreeMap::new();

    for (linenum, line) in clean_lines.elided.iter().enumerate() {
        if clean_lines.raw_lines[linenum].contains("NOLINT") {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut matched_headers = HashSet::new();
        for mat in IWYU_AC.find_iter(line) {
            let start = mat.start();
            let end = mat.end();
            let check = &IWYU_CHECKS[mat.pattern()];
            if matched_headers.contains(check.header) {
                continue;
            }

            let m = IwyuMatch { line, start, end };
            match check.kind {
                IwyuKind::Word => {
                    if m.is_word_match() {
                        required.insert(check.header, (linenum, check.token.to_string()));
                        matched_headers.insert(check.header);
                    }
                }
                IwyuKind::FuncOrTempl => {
                    if m.is_function_or_template_match() {
                        required.insert(check.header, (linenum, check.token.to_string()));
                        matched_headers.insert(check.header);
                    }
                }
                IwyuKind::StdTempl => {
                    if m.is_std_template_match() {
                        required.insert(check.header, (linenum, format!("{}<>", check.token)));
                        matched_headers.insert(check.header);
                    }
                }
                IwyuKind::Templ => {
                    if m.is_template_match() {
                        required.insert(check.header, (linenum, format!("{}<>", check.token)));
                        matched_headers.insert(check.header);
                    }
                }
                IwyuKind::Func => {
                    if m.is_function_match() {
                        required.insert(check.header, (linenum, check.token.to_string()));
                        matched_headers.insert(check.header);
                    }
                }
            }
        }
    }

    for (header, (linenum, symbol)) in required {
        if include_state.find_header(header).is_none() {
            linter.error(
                linenum,
                "build/include_what_you_use",
                4,
                &format!("Add #include <{}> for {}", header, symbol),
            );
        }
    }
}

struct IwyuMatch<'a> {
    line: &'a str,
    start: usize,
    end: usize,
}

impl<'a> IwyuMatch<'a> {
    fn is_word_match(&self) -> bool {
        self.match_start(|line, end| {
            end == line.len() || !is_iwyu_word_char(line[end..].chars().next().unwrap_or('\0'))
        })
    }

    fn is_function_match(&self) -> bool {
        self.match_start(|line, end| {
            let index = skip_spaces(line, end);
            line[index..]
                .strip_prefix('(')
                .and_then(|rest| rest.chars().next())
                .is_some_and(|ch| ch != ')')
        })
    }

    fn is_template_match(&self) -> bool {
        let prev = self.line[..self.start].chars().next_back();
        if prev.is_some_and(is_iwyu_word_char) {
            return false;
        }
        if !prefix_allows_template_iwyu(&self.line[..self.start]) {
            return false;
        }
        next_non_space_char(self.line, self.end) == Some('<')
    }

    fn is_std_template_match(&self) -> bool {
        self.line[..self.start].ends_with("std::")
            && next_non_space_char(self.line, self.end) == Some('<')
    }

    fn is_function_or_template_match(&self) -> bool {
        self.match_start(|line, end| {
            let mut index = skip_spaces(line, end);
            if line[index..].starts_with('<') {
                index += 1;
                let mut depth = 1usize;
                while index < line.len() {
                    match line.as_bytes()[index] {
                        b'<' => depth += 1,
                        b'>' => {
                            depth -= 1;
                            if depth == 0 {
                                index += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    index += 1;
                }
            }
            let index = skip_spaces(line, index);
            line[index..]
                .strip_prefix('(')
                .and_then(|rest| rest.chars().next())
                .is_some_and(|ch| ch != ')')
        })
    }

    fn match_start<F>(&self, suffix_matches: F) -> bool
    where
        F: Fn(&str, usize) -> bool,
    {
        let prev = self.line[..self.start].chars().next_back();
        if prev.is_some_and(is_iwyu_word_char) {
            return false;
        }
        let prefix = &self.line[..self.start];
        if !prefix_allows_iwyu(prefix) {
            return false;
        }
        suffix_matches(self.line, self.end)
    }
}

fn prefix_allows_iwyu(prefix: &str) -> bool {
    prefix.ends_with("std::")
        || (!prefix.ends_with("::")
            && !prefix.ends_with('.')
            && !prefix.ends_with("->")
            && !prefix.ends_with('>'))
}

fn prefix_allows_template_iwyu(prefix: &str) -> bool {
    if let Some(before_std) = prefix.strip_suffix("std::") {
        return before_std.is_empty()
            || before_std.ends_with("::")
            || before_std
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_whitespace());
    }

    prefix
        .chars()
        .next_back()
        .is_none_or(|ch| ch != '>' && ch != '.' && ch != ':')
}

fn skip_spaces(line: &str, mut index: usize) -> usize {
    while index < line.len() && line.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn next_non_space_char(line: &str, index: usize) -> Option<char> {
    line[skip_spaces(line, index)..].chars().next()
}

fn is_iwyu_word_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn check_header_file_included(linter: &mut FileLinter, include_state: &IncludeState) {
    let file_path = linter.file_path();
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if linter.options().header_extensions().contains(extension) {
        return;
    }

    let stem = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    if stem.ends_with("_test") || stem.ends_with("_regtest") || stem.ends_with("_unittest") {
        return;
    }

    let Some(directory) = file_path.parent() else {
        return;
    };
    let file_from_repo = linter.relative_from_repository();
    let path_from_repo = file_from_repo.to_string_lossy().replace('\\', "/");
    let mut first_include_line = None;
    let mut includes_use_aliases = false;
    for section_list in include_state.include_lists() {
        for (include, line) in section_list {
            if first_include_line.is_none() {
                first_include_line = Some(*line);
            }
            if include.contains("./") || include.contains("../") {
                includes_use_aliases = true;
            }
        }
    }

    for header_ext in linter.options().header_extensions() {
        let header_path = directory.join(format!("{}.{}", stem, header_ext));
        if !header_path.is_file() {
            continue;
        }

        let mut header_name = linter
            .relative_from_repository()
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("{}.{}", stem, header_ext))
            .to_string_lossy()
            .replace('\\', "/");
        if header_name.is_empty() {
            header_name = format!("{}.{}", stem, header_ext);
        }

        let found = include_state.include_lists().iter().any(|section_list| {
            section_list
                .iter()
                .any(|(include, _)| header_name.contains(include) || include.contains(&header_name))
        });
        if found {
            return;
        }

        let mut message = format!(
            "{} should include its header file {}",
            path_from_repo, header_name
        );
        if includes_use_aliases {
            message.push_str(". Relative paths like . and .. are not allowed.");
        }
        linter.error(
            first_include_line.unwrap_or(0),
            "build/include",
            5,
            &message,
        );
        return;
    }
}

fn generate_guard(path: &Path) -> String {
    let mut parts = Vec::new();

    for component in path.components() {
        if let Some(part) = component.as_os_str().to_str()
            && !part.is_empty() && part != "." {
                parts.push(part);
            }
    }

    let joined = if parts.is_empty() {
        path.to_string_lossy().to_string()
    } else {
        parts.join("_")
    };
    let mut guard = joined
        .replace(|c: char| !c.is_alphanumeric(), "_")
        .to_uppercase();
    if !guard.ends_with('_') {
        guard.push('_');
    }
    guard
}
