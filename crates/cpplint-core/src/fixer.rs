use crate::c_headers;
use crate::categories::Category;
use crate::cleanse::CleansedLines;
use crate::diagnostics::Diagnostic;
use crate::errors::Result;
use crate::facts::FileFacts;
use crate::file_linter::FileLinter;
use crate::file_reader;
use crate::line_utils;
use crate::options::{IncludeOrder, Options};
use crate::state::CppLintState;
use crate::state::IncludeKind;
use crate::string_utils;
use crate::syntax::{ParsedLine, base_name};
use bumpalo::Bump;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::cell::{RefCell, UnsafeCell};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

thread_local! {
    static FIXER_ARENA: UnsafeCell<Bump> = UnsafeCell::new(Bump::new());
    static OPERATOR_SPACE_REGEX_CACHE: RefCell<FxHashMap<String, Regex>> =
        RefCell::new(FxHashMap::default());
}
use std::sync::LazyLock;

const MAX_FIX_PASSES: usize = 8;

static COMMENT_SPLIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(?P<code>.*?)(?P<comment>//.*)$"#).unwrap());
static TODO_FIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"//\s*TODO\(([^)]+)\):?\s*(.*)$"#).unwrap());
static ENDIF_TEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(\s*#\s*endif)\s+([^/\s].*)$"#).unwrap());
static ALT_TOKEN_FIXES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"\band_eq\b", "&="),
        (r"\bor_eq\b", "|="),
        (r"\bnot_eq\b", "!="),
        (r"\bbitand\b", "&"),
        (r"\bbitor\b", "|"),
        (r"\bcompl\b", "~"),
        (r"\bxor_eq\b", "^="),
        (r"\bxor\b", "^"),
        (r"\band\b", "&&"),
        (r"\bor\b", "||"),
        (r"\bnot\b", "!"),
    ]
    .into_iter()
    .map(|(pattern, replacement)| (Regex::new(pattern).unwrap(), replacement))
    .collect()
});
static REDUNDANT_SPACE_AFTER_SLASHES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^//(?P<body>\S.*)$"#).unwrap());
static COMMA_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#",([^,\s])"#).unwrap());
static BRACE_SEMICOLON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"}\s*;\s*$"#).unwrap());
static SEMICOLON_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#";([^\s};\\)/])"#).unwrap());
static COLON_SEMICOLON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#":\s*;\s*$"#).unwrap());
static SPACE_SEMICOLON_ANY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\s+;"#).unwrap());
static SPACE_SEMICOLON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\s+;\s*$"#).unwrap());
static PRINTF_Q_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"%([-+ 0#]*\d*(?:\.\d+)?)q"#).unwrap());
static ACCESS_SPECIFIER_FIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?P<access>public|private|protected)(?P<slots>\s+slots)?:(?P<suffix>.*)$"#)
        .unwrap()
});
static EMPTY_BODY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\s*;\s*$"#).unwrap());
static STORAGE_CLASS_FIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(?P<indent>\s*)(?P<prefix>.+?)\b(?P<storage>thread_local|static|extern|typedef|register|auto|mutable)\b(?P<suffix>\s+.+)$"#,
    )
    .unwrap()
});
static GLOBAL_CONST_STRING_FIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(?P<indent>\s*)(?P<prefix>(?:static\s+)?(?:const\s+)?)((?:::\s*)?(?:std::)?string)(?P<suffix_const>\s+const)?\s+(?P<name>[a-zA-Z0-9_:]+)\b(?P<rest>\s*=.*)$"#,
    )
    .unwrap()
});
static GLOBAL_CONST_STRING_DIRECT_INIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(?P<indent>\s*)(?P<prefix>(?:static\s+)?(?:const\s+)?)((?:::\s*)?(?:std::)?string)(?P<suffix_const>\s+const)?\s+(?P<name>[a-zA-Z0-9_:]+)\s*(?P<open>[\(\{])(?P<init>.*)(?P<close>[\)\}])(?P<suffix>\s*;.*)$"#,
    )
    .unwrap()
});
static POINTER_INCREMENT_FIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(?P<indent>\s*)\*(?P<name>[A-Za-z_]\w*)(?P<op>\+\+|--)(?P<suffix>\s*;.*)$"#)
        .unwrap()
});

// ⚡ Bolt: Extracted dynamically compiled regular expressions into lazy static variables.
// Regex compilation is expensive and these functions are called frequently in a hot path.
// This optimization ensures each regex is compiled exactly once, improving performance
// by ~6.7% across the macro/quantlib benchmarks.
static BRACE_SPACE_BEFORE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([A-Za-z0-9_&])\s+\["#).unwrap());
static BRACE_MISSING_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([^ ({>])\{"#).unwrap());
static PAREN_SPACE_FUNC_CALL_BEFORE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([A-Za-z_~][\w:]*)\s+\("#).unwrap());
static PAREN_SPACE_AFTER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\(\s+"#).unwrap());
static PAREN_SPACE_BEFORE_CLOSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+\)"#).unwrap());
static INHERITANCE_VIRTUAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvirtual\s+"#).unwrap());
static INHERITANCE_OVERRIDE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boverride\s+"#).unwrap());
static MEMSET_FIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"memset\s*\(([^,]*),\s*([^,]*),\s*0\s*\)"#).unwrap());
static UNARY_NOT_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"!\s+"#).unwrap());
static UNARY_COMPL_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"~\s+"#).unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewlineStyle {
    Lf,
    CrLf,
}

pub fn fix_file_in_place(path: &Path, options: &Options) -> Result<bool> {
    if path == Path::new("-") {
        return Ok(false);
    }

    let raw_bytes = std::fs::read(path)?;
    let had_utf8_bom = raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let read_result = file_reader::read_lines(path)?;
    if !read_result.invalid_utf8_lines.is_empty() {
        return Ok(false);
    }

    let lines_vec = read_result.to_lines_vec();
    let mixed_line_endings = has_mixed_line_endings(
        &lines_vec,
        read_result.lf_lines_count,
        &read_result.crlf_lines,
    );
    let mut lines = lines_vec;
    let original_lines = lines.clone();
    let newline_style = if mixed_line_endings || read_result.crlf_lines.is_empty() {
        NewlineStyle::Lf
    } else {
        NewlineStyle::CrLf
    };

    for _ in 0..MAX_FIX_PASSES {
        let diagnostics = lint_lines(path, options, &lines);
        if diagnostics.is_empty() {
            break;
        }

        let mut changed = false;
        changed |= fix_header_guard(path, options, &diagnostics, &mut lines);
        changed |= fix_include_block(path, options, &diagnostics, &mut lines);
        changed |= fix_namespace_comments(&diagnostics, &mut lines);
        changed |= fix_brace_placement(&diagnostics, &mut lines);
        if changed {
            continue;
        }

        changed |= apply_line_fixes(path, options, &diagnostics, &mut lines);
        if !changed {
            break;
        }
    }

    let should_write = mixed_line_endings || lines != original_lines;
    if !should_write {
        return Ok(false);
    }

    write_lines(path, &lines, newline_style, had_utf8_bom)?;
    Ok(true)
}

fn lint_lines(path: &Path, options: &Options, lines: &[String]) -> Vec<Diagnostic> {
    let state = CppLintState::new();
    let mut linter = FileLinter::new(path.to_path_buf(), &state, options.clone());
    linter.process_file_data(lines);
    state.diagnostics()
}

fn has_mixed_line_endings(lines: &[String], lf_lines_count: usize, crlf_lines: &[usize]) -> bool {
    let lf_count = if !lines.is_empty()
        && lines.last().is_some_and(|line| line.is_empty())
        && lf_lines_count > 0
    {
        lf_lines_count - 1
    } else {
        lf_lines_count
    };
    lf_count > 0 && !crlf_lines.is_empty()
}

fn write_lines(
    path: &Path,
    lines: &[String],
    newline_style: NewlineStyle,
    had_utf8_bom: bool,
) -> Result<()> {
    let separator = match newline_style {
        NewlineStyle::Lf => "\n",
        NewlineStyle::CrLf => "\r\n",
    };
    let mut contents = lines.join(separator);
    if had_utf8_bom {
        contents.insert(0, '\u{FEFF}');
    }
    std::fs::write(path, contents)?;
    Ok(())
}

fn fix_header_guard(
    path: &Path,
    options: &Options,
    diagnostics: &[Diagnostic],
    lines: &mut Vec<String>,
) -> bool {
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.category.as_str() == "build/header_guard")
    {
        return false;
    }
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if !options.header_extensions().contains(extension) {
        return false;
    }
    if lines.iter().any(|line| line.trim() == "#pragma once") {
        return false;
    }

    let expected_guard = expected_header_guard(path, options);
    let mut ifndef = None;
    let mut define = None;
    let mut endif = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#ifndef ") {
            if ifndef.is_none() {
                ifndef = Some((idx, rest.trim().to_string()));
            }
        } else if let Some(rest) = trimmed.strip_prefix("#define ") {
            if define.is_none() {
                define = Some((idx, rest.trim().to_string()));
            }
        } else if trimmed.starts_with("#endif") {
            endif = Some(idx);
        }
    }

    let mut changed = false;
    if let (Some((ifndef_idx, _)), Some((define_idx, _))) = (ifndef, define) {
        let expected_ifndef = format!("#ifndef {}", expected_guard);
        let expected_define = format!("#define {}", expected_guard);
        if lines[ifndef_idx] != expected_ifndef {
            lines[ifndef_idx] = expected_ifndef;
            changed = true;
        }
        if lines[define_idx] != expected_define {
            lines[define_idx] = expected_define;
            changed = true;
        }

        let endif_idx = endif.unwrap_or_else(|| {
            lines.push(String::new());
            lines.len() - 1
        });
        let expected_endif = format!("#endif  // {}", expected_guard);
        if lines[endif_idx].trim() != expected_endif {
            lines[endif_idx] = expected_endif;
            changed = true;
        }
        return changed;
    }

    let insertion = header_guard_insertion_index(lines);
    lines.insert(insertion, format!("#ifndef {}", expected_guard));
    lines.insert(insertion + 1, format!("#define {}", expected_guard));
    let endif_insert_at = if lines.last().is_some_and(|line| line.is_empty()) {
        lines.len() - 1
    } else {
        lines.len()
    };
    lines.insert(endif_insert_at, format!("#endif  // {}", expected_guard));
    true
}

fn header_guard_insertion_index(lines: &[String]) -> usize {
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.is_empty() {
            idx += 1;
            continue;
        }
        if trimmed.starts_with("//") {
            idx += 1;
            continue;
        }
        if trimmed.starts_with("/*") {
            idx += 1;
            while idx < lines.len() && !lines[idx - 1].contains("*/") {
                idx += 1;
            }
            continue;
        }
        break;
    }
    idx
}

fn fix_include_block(
    path: &Path,
    options: &Options,
    diagnostics: &[Diagnostic],
    lines: &mut Vec<String>,
) -> bool {
    let additions = missing_include_entries_from_diagnostics(path, options, diagnostics);
    let relevant = diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.category,
            Category::BuildInclude
                | Category::BuildIncludeAlpha
                | Category::BuildIncludeOrder
                | Category::BuildIncludeWhatYouUse
        )
    });
    if !relevant && additions.is_empty() {
        return false;
    }

    let (start, end) = if let Some(range) = top_level_include_block(lines) {
        range
    } else if additions.is_empty() {
        return false;
    } else {
        let insert_at = include_block_insertion_index(lines);
        (insert_at, insert_at)
    };
    if lines[start..end].iter().any(|line| {
        matches!(
            preprocessor_directive(line.trim()),
            Some("if" | "ifdef" | "ifndef" | "else" | "elif" | "endif")
        )
    }) {
        return false;
    }

    let file_from_repo = relative_from_repository(path, &options.repository);
    let mut seen = FxHashSet::default();
    let mut entries = Vec::new();
    for line in &lines[start..end] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((delimiter, include)) = string_utils::parse_include_directive(trimmed) else {
            continue;
        };
        if !seen.insert(include.to_string()) {
            continue;
        }
        entries.push(IncludeEntry {
            include: include.to_string(),
            raw_line: format!(
                "#include {}{}{}",
                delimiter,
                include,
                if delimiter == "<" { ">" } else { "\"" }
            ),
            kind: classify_include(
                &file_from_repo,
                Path::new(&include),
                delimiter == "<",
                options.include_order,
            ),
            alpha_key: canonicalize_alpha(include),
        });
    }

    for entry in additions {
        if seen.insert(entry.include.clone()) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        return false;
    }

    entries.sort_by(|lhs, rhs| {
        include_kind_rank(lhs.kind)
            .cmp(&include_kind_rank(rhs.kind))
            .then_with(|| lhs.alpha_key.cmp(&rhs.alpha_key))
            .then_with(|| lhs.include.cmp(&rhs.include))
    });

    let replacement: Vec<String> = entries.into_iter().map(|entry| entry.raw_line).collect();
    let current: Vec<String> = lines[start..end]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .cloned()
        .collect();
    if current == replacement {
        return false;
    }

    lines.splice(start..end, replacement);
    true
}

fn top_level_include_block(lines: &[String]) -> Option<(usize, usize)> {
    let mut start = None;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if string_utils::parse_include_directive(trimmed).is_some() {
            start = Some(idx);
            break;
        }
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("#ifndef ")
            || trimmed.starts_with("#define ")
            || trimmed == "#pragma once"
        {
            continue;
        }
        if !trimmed.is_empty() {
            return None;
        }
    }
    let start = start?;
    let mut end = start;
    while end < lines.len() {
        let trimmed = lines[end].trim();
        if trimmed.is_empty() || string_utils::parse_include_directive(trimmed).is_some() {
            end += 1;
            continue;
        }
        break;
    }
    Some((start, end))
}

fn include_block_insertion_index(lines: &[String]) -> usize {
    let mut idx = header_guard_insertion_index(lines);
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.is_empty()
            || trimmed.starts_with("#ifndef ")
            || trimmed.starts_with("#define ")
            || trimmed == "#pragma once"
        {
            idx += 1;
            continue;
        }
        break;
    }
    idx
}

fn missing_include_entries_from_diagnostics(
    path: &Path,
    options: &Options,
    diagnostics: &[Diagnostic],
) -> Vec<IncludeEntry> {
    let file_from_repo = relative_from_repository(path, &options.repository);
    let mut entries = Vec::new();

    if let Some(header_name) = missing_self_header_from_diagnostics(diagnostics) {
        entries.push(IncludeEntry {
            raw_line: format!("#include \"{}\"", header_name),
            kind: IncludeKind::LikelyMyHeader,
            alpha_key: canonicalize_alpha(&header_name),
            include: header_name,
        });
    }

    for header in missing_iwyu_headers_from_diagnostics(diagnostics) {
        let header_str = header.as_str();
        entries.push(IncludeEntry {
            raw_line: format!("#include <{}>", header_str),
            kind: classify_include(
                &file_from_repo,
                Path::new(header_str),
                true,
                options.include_order,
            ),
            alpha_key: canonicalize_alpha(header_str),
            include: header_str.to_string(),
        });
    }

    entries
}

fn missing_self_header_from_diagnostics(diagnostics: &[Diagnostic]) -> Option<String> {
    use crate::messages::LintMessage;
    diagnostics.iter().find_map(|diagnostic| {
        if let LintMessage::MissingSelfHeader { header, .. } = &diagnostic.message {
            Some(header.to_string())
        } else {
            None
        }
    })
}

fn missing_iwyu_headers_from_diagnostics(
    diagnostics: &[Diagnostic],
) -> Vec<crate::iwyu::IwyuHeader> {
    use crate::messages::LintMessage;
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            if let LintMessage::IwyuAddInclude(header, _) = &diagnostic.message {
                Some(*header)
            } else {
                None
            }
        })
        .collect()
}

fn fix_namespace_comments(diagnostics: &[Diagnostic], lines: &mut [String]) -> bool {
    use crate::messages::LintMessage;
    let mut changed = false;
    for diagnostic in diagnostics {
        if diagnostic.category.as_str() != "readability/namespace" {
            continue;
        }
        let idx = diagnostic.linenum.saturating_sub(1);
        if idx >= lines.len() {
            continue;
        }
        let replacement = if let LintMessage::NamespaceMissingComment(name) = &diagnostic.message {
            format!("}}  // namespace {}", name)
        } else {
            "}  // namespace".to_string()
        };
        if !lines[idx].trim_start().starts_with('}') {
            continue;
        }
        if lines[idx].trim() != replacement {
            let indent = lines[idx]
                .chars()
                .take_while(|ch| ch.is_ascii_whitespace())
                .collect::<String>();
            lines[idx] = format!("{}{}", indent, replacement);
            changed = true;
        }
    }
    changed
}

fn fix_brace_placement(diagnostics: &[Diagnostic], lines: &mut Vec<String>) -> bool {
    use crate::messages::LintMessage;
    let mut targets: Vec<usize> = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                &diagnostic.message,
                LintMessage::MissingSpaceBeforeOpenBrace
            )
        })
        .map(|diagnostic| diagnostic.linenum.saturating_sub(1))
        .collect();
    if targets.is_empty() {
        return false;
    }
    targets.sort_unstable();
    targets.dedup();
    let mut changed = false;
    for idx in targets.into_iter().rev() {
        if idx == 0 || idx >= lines.len() || lines[idx].trim() != "{" {
            continue;
        }
        let Some(prev_idx) = previous_non_blank_line(lines, idx) else {
            continue;
        };
        let merged = format!("{} {{", lines[prev_idx].trim_end());
        if lines[prev_idx] != merged {
            lines[prev_idx] = merged;
        }
        lines.remove(idx);
        changed = true;
    }
    changed
}

fn apply_line_fixes(
    path: &Path,
    options: &Options,
    diagnostics: &[Diagnostic],
    lines: &mut Vec<String>,
) -> bool {
    use crate::messages::LintMessage;
    let mut ordered: Vec<_> = diagnostics.iter().collect();
    ordered.sort_by(|lhs, rhs| {
        rhs.linenum
            .cmp(&lhs.linenum)
            .then_with(|| lhs.category.cmp(&rhs.category))
    });

    FIXER_ARENA.with(|arena_cell| {
        // SAFETY: The arena is thread-local and used synchronously within apply_line_fixes.
        let arena = unsafe { &mut *arena_cell.get() };
        let mut facts_cache = FactsCache::new(arena);
        let mut changed = false;
        for diagnostic in ordered {
            match &diagnostic.message {
                LintMessage::TabFound => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        let fixed = line.replace('\t', "  ");
                        if *line != fixed {
                            *line = fixed;
                            changed = true;
                        }
                    }
                }
                LintMessage::TrailingWhitespace => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        let fixed = line.trim_end().to_string();
                        if *line != fixed {
                            *line = fixed;
                            changed = true;
                        }
                    }
                }
                LintMessage::NewlineShouldBeAtEndOfFile
                    if lines.last().is_some_and(|line| !line.is_empty()) =>
                {
                    lines.push(String::new());
                    changed = true;
                }
                m @ (LintMessage::AtLeastTwoSpacesBetweenCodeAndComments
                | LintMessage::ShouldHaveSpaceBetweenSlashesAndComment) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_comment_spacing(line, m);
                    }
                }
                m @ (LintMessage::MultipleBlankLines
                | LintMessage::BlankLineAtStartOfBlock
                | LintMessage::BlankLineAtEndOfBlock
                | LintMessage::NoBlankLineAfterSection) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    changed |= fix_blank_line(lines, idx, m);
                }
                LintMessage::MissingUsernameInTodo | LintMessage::TodoShouldBeFollowedBySpace => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_todo_spacing(line);
                    }
                }
                LintMessage::MissingSpaceAfterComma => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= update_code_and_comment(line, |code| {
                            COMMA_SPACE_RE.replace_all(code, ", $1").into_owned()
                        });
                    }
                }
                m @ (LintMessage::ExtraSpaceBeforeSemicolon
                | LintMessage::MissingSpaceBeforeSemicolon
                | LintMessage::ExtraSpaceAfterSemicolon) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_semicolon_spacing(line, m);
                    }
                }
                LintMessage::ExtraSpaceForOperator(crate::messages::OperatorSymbol::Colon) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_range_for_colon(line);
                    }
                }
                m @ (LintMessage::MissingSpaceBeforeOpenBrace
                | LintMessage::MissingSpaceBeforeElse) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_brace_spacing(line, m);
                    }
                }
                LintMessage::BracesMissing(kind)
                    if kind.as_ref() == "if"
                        || kind.as_ref() == "while"
                        || kind.as_ref() == "for" =>
                {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_empty_control_body(line, &[kind.as_ref()]);
                    }
                }
                LintMessage::EmptyIfBody
                | LintMessage::EmptyLoopBody
                | LintMessage::EmptyConditionalBody => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if idx < lines.len() {
                        changed |= fix_empty_if_body(lines, idx);
                    }
                }
                m @ (LintMessage::ExtraSpaceAfterParen
                | LintMessage::ExtraSpaceAfterParenInFuncCall
                | LintMessage::MismatchingSpacesInsideParen
                | LintMessage::MissingSpaceBeforeOpenParen
                | LintMessage::ExtraSpaceBeforeCloseParen
                | LintMessage::ClosingParenShouldBeMovedToPreviousLine) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if idx < lines.len() {
                        changed |= fix_paren_spacing(lines, idx, m);
                    }
                }
                LintMessage::NamespaceIndented => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_namespace_indentation(line);
                    }
                }
                LintMessage::ShouldBeIndented(msg) if msg.contains("+1 space inside") => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if idx < lines.len() {
                        let (clean_lines, facts) = facts_cache.get(path, options, lines);
                        changed |= fix_access_specifier_indentation_with_facts(
                            lines,
                            idx,
                            clean_lines,
                            facts,
                        );
                    }
                }
                LintMessage::ClosingBraceAlignment(_) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if idx < lines.len() {
                        let (clean_lines, facts) = facts_cache.get(path, options, lines);
                        changed |= fix_class_closing_brace_alignment_with_facts(
                            lines,
                            idx,
                            clean_lines,
                            facts,
                        );
                    }
                }
                m @ (LintMessage::MissingSpacesAround(_)
                | LintMessage::ExtraSpaceForOperator(_)) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_operator_spacing(line, m);
                    }
                }
                LintMessage::AltToken(_, _) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_alt_tokens(line);
                    }
                }
                LintMessage::CheckMacroSuggestion { .. } => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_check_macro(line);
                    }
                }
                LintMessage::ConstructorShouldBeExplicit(_) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_constructor_should_be_explicit(line);
                    }
                }
                LintMessage::DeprecatedCastingStyle(type_str) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_deprecated_cast_style(line, type_str);
                    }
                }
                LintMessage::CStyleCast(cast_type, type_str) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_c_style_cast(line, cast_type, type_str);
                    }
                }
                LintMessage::ChangingPointerInsteadOfValue => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_invalid_increment(line);
                    }
                }
                LintMessage::RedundantStringCtor => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_redundant_string_ctor(line);
                    }
                }
                LintMessage::UnnecessarySemicolonAfterBrace => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        let fixed = BRACE_SEMICOLON_RE.replace(line, "}").into_owned();
                        if *line != fixed {
                            *line = fixed;
                            changed = true;
                        }
                    }
                }
                m @ (LintMessage::RedundantVirtual | LintMessage::RedundantOverride) => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if idx < lines.len() {
                        changed |= fix_inheritance_redundancy(lines, idx, m);
                    }
                }
                _m @ (LintMessage::EndifCommentMissing(_) | LintMessage::EndifLineShouldBe(_))
                    if diagnostic.category.as_str() == "build/endif_comment" =>
                {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_endif_comment(line);
                    }
                }
                LintMessage::PrintfFormatDeprecatedQ => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_printf_format(line, &diagnostic.message);
                    }
                }
                LintMessage::BuildExplicitMakePair => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_make_pair(line);
                    }
                }
                LintMessage::VlogShouldUseNumericVerbosityLevel => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_vlog_macro(line);
                    }
                }
                LintMessage::GlobalStringConstantSuggestion {
                    prefix,
                    suffix_const,
                    name,
                } => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_global_const_string(line, prefix, suffix_const, name);
                    }
                }
                LintMessage::SnprintfSizeofSuggestion { buffer, size } => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_snprintf_sizeof(line, buffer, size);
                    }
                }
                LintMessage::PotentialFormatStringBug { function, arg } => {
                    let idx = diagnostic.linenum.saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        changed |= fix_potential_format_string_bug(line, function, arg);
                    }
                }
                _ => {
                    // Fall back to category-based matching for remaining legacy fixes or Raw messages
                    match diagnostic.category.as_str() {
                        "build/explicit_make_pair" => {
                            let idx = diagnostic.linenum.saturating_sub(1);
                            if let Some(line) = lines.get_mut(idx) {
                                changed |= fix_make_pair(line);
                            }
                        }
                        "build/storage_class" => {
                            let idx = diagnostic.linenum.saturating_sub(1);
                            if let Some(line) = lines.get_mut(idx) {
                                changed |= fix_storage_class(line);
                            }
                        }
                        "runtime/memset" => {
                            let idx = diagnostic.linenum.saturating_sub(1);
                            if let Some(line) = lines.get_mut(idx) {
                                changed |= fix_memset(line);
                            }
                        }
                        "runtime/vlog" => {
                            let idx = diagnostic.linenum.saturating_sub(1);
                            if let Some(line) = lines.get_mut(idx) {
                                changed |= fix_vlog_macro(line);
                            }
                        }
                        "runtime/printf_format" | "build/printf_format" => {
                            let idx = diagnostic.linenum.saturating_sub(1);
                            if let Some(line) = lines.get_mut(idx) {
                                changed |= fix_printf_format(line, &diagnostic.message);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        changed
    })
}

fn fix_blank_line(
    lines: &mut Vec<String>,
    idx: usize,
    message: &crate::messages::LintMessage,
) -> bool {
    use crate::messages::LintMessage;
    if idx >= lines.len() {
        return false;
    }
    if matches!(
        message,
        LintMessage::MultipleBlankLines
            | LintMessage::BlankLineAtStartOfBlock
            | LintMessage::BlankLineAtEndOfBlock
            | LintMessage::NoBlankLineAfterSection
    ) && lines[idx].trim().is_empty()
    {
        lines.remove(idx);
        return true;
    }
    // Note: Some blank line fixes are not easily mapped to a single variant yet
    // but the above covers the main cases.
    false
}

fn fix_comment_spacing(line: &mut String, message: &crate::messages::LintMessage) -> bool {
    use crate::messages::LintMessage;
    let Some(comment_idx) = find_line_comment_start(line) else {
        return false;
    };
    let code = &line[..comment_idx];
    let comment = &line[comment_idx..];

    let fixed = match message {
        LintMessage::AtLeastTwoSpacesBetweenCodeAndComments => {
            if code.trim().is_empty() {
                return false;
            }
            format!("{}  {}", code.trim_end(), comment)
        }
        LintMessage::ShouldHaveSpaceBetweenSlashesAndComment => {
            if comment.starts_with("///")
                || comment.starts_with("//!")
                || comment.starts_with("// ")
            {
                return false;
            }
            if let Some(captures) = REDUNDANT_SPACE_AFTER_SLASHES_RE.captures(comment) {
                format!(
                    "{}// {}",
                    code,
                    captures.name("body").map(|m| m.as_str()).unwrap_or("")
                )
            } else {
                return false;
            }
        }
        _ => return false,
    };

    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_todo_spacing(line: &mut String) -> bool {
    let Some(comment_idx) = find_line_comment_start(line) else {
        return false;
    };
    let code = &line[..comment_idx];
    let comment = &line[comment_idx..];
    let Some(captures) = TODO_FIX_RE.captures(comment) else {
        return false;
    };
    let user = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if user.is_empty() {
        return false;
    }
    let rest = captures
        .get(2)
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim_start();
    let suffix = if rest.is_empty() {
        String::new()
    } else {
        format!(" {}", rest)
    };
    let fixed = format!("{}// TODO({}):{}", code, user, suffix);
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_empty_control_body(line: &mut String, keywords: &[&str]) -> bool {
    update_code_and_comment(line, |code| {
        let trimmed = code.trim_start();
        if !keywords
            .iter()
            .any(|keyword| starts_with_keyword(trimmed, keyword))
        {
            return code.to_string();
        }
        let Some(semicolon) = code.rfind(';') else {
            return code.to_string();
        };
        if !code[semicolon + 1..].trim().is_empty() {
            return code.to_string();
        }
        let Some(close_paren) = code.rfind(')') else {
            return code.to_string();
        };
        if close_paren > semicolon {
            return code.to_string();
        }
        format!("{} {{}}", code[..semicolon].trim_end())
    })
}

fn starts_with_keyword(line: &str, keyword: &str) -> bool {
    let Some(rest) = line.strip_prefix(keyword) else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_whitespace() || ch == '(')
}

fn fix_empty_if_body(lines: &mut Vec<String>, idx: usize) -> bool {
    let line = &lines[idx];
    if line.trim().ends_with(';') {
        let fixed = EMPTY_BODY_RE.replace(line, " {}").into_owned();
        if *line != fixed {
            lines[idx] = fixed;
            return true;
        }
        return false;
    }

    // Find the opening brace
    let mut opening_idx = idx;
    while opening_idx < lines.len() && !lines[opening_idx].contains('{') {
        opening_idx += 1;
    }
    if opening_idx >= lines.len() {
        return false;
    }

    // Find the closing brace
    let mut depth = 0;
    let mut closing_idx = opening_idx;
    let mut found = false;
    while closing_idx < lines.len() {
        let line = &lines[closing_idx];
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    found = true;
                    break;
                }
            }
        }
        if found {
            break;
        }
        closing_idx += 1;
    }

    if !found {
        return false;
    }

    // Check if it's empty
    let mut is_empty = true;
    for (i, line) in lines
        .iter()
        .enumerate()
        .take(closing_idx + 1)
        .skip(opening_idx)
    {
        let l = if i == opening_idx {
            let start = line.find('{').unwrap();
            &line[start + 1..]
        } else if i == closing_idx {
            let end = line.find('}').unwrap();
            &line[..end]
        } else {
            line
        };
        if !l.trim().is_empty() {
            is_empty = false;
            break;
        }
    }

    if is_empty {
        let _indent = lines[idx]
            .chars()
            .take_while(|ch| ch.is_ascii_whitespace())
            .collect::<String>();
        let mut new_line = lines[idx].clone();
        if let Some(brace_pos) = new_line.find('{') {
            new_line.truncate(brace_pos);
        } else if let Some(semi_pos) = new_line.find(';') {
            new_line.truncate(semi_pos);
        }
        lines[idx] = format!("{} {{}}", new_line.trim_end());
        lines.drain(idx + 1..=closing_idx);
        return true;
    }

    false
}

fn fix_semicolon_spacing(line: &mut String, message: &crate::messages::LintMessage) -> bool {
    use crate::messages::LintMessage;
    let fixed = match message {
        LintMessage::MissingSpaceBeforeSemicolon => update_code(line, |code| {
            SEMICOLON_SPACE_RE.replace_all(code, "; $1").into_owned()
        }),
        LintMessage::SemicolonDefiningEmptyStatementUseBraces => {
            COLON_SEMICOLON_RE.replace(line, ": {}").into_owned()
        }
        LintMessage::LineContainsOnlySemicolonUseBraces => {
            let indent = line
                .chars()
                .take_while(|ch| ch.is_ascii_whitespace())
                .collect::<String>();
            format!("{}{{}}", indent)
        }
        LintMessage::ExtraSpaceBeforeSemicolon => update_code(line, |code| {
            SPACE_SEMICOLON_ANY_RE.replace_all(code, ";").into_owned()
        }),
        LintMessage::ExtraSpaceBeforeLastSemicolonUseBraces => {
            SPACE_SEMICOLON_RE.replace(line, ";").into_owned()
        }
        _ => return false,
    };
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_range_for_colon(line: &mut String) -> bool {
    let Some(start) = line.find("for") else {
        return false;
    };
    let Some(open_offset) = line[start..].find('(') else {
        return false;
    };
    let open = start + open_offset;
    let Some(close) = find_matching_paren(line, open) else {
        return false;
    };
    let inside = &line[open + 1..close];
    let mut depth = 0usize;
    for (idx, ch) in inside.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => {
                let bytes = inside.as_bytes();
                if idx > 0 && bytes[idx - 1] == b':' {
                    continue;
                }
                if idx + 1 < bytes.len() && bytes[idx + 1] == b':' {
                    continue;
                }
                let lhs = inside[..idx].trim_end();
                let rhs = inside[idx + 1..].trim_start();
                let mut fixed = String::new();
                fixed.push_str(&line[..open + 1]);
                fixed.push_str(lhs);
                fixed.push_str(" : ");
                fixed.push_str(rhs);
                fixed.push_str(&line[close..]);
                if *line != fixed {
                    *line = fixed;
                    return true;
                }
                return false;
            }
            _ => {}
        }
    }
    false
}

fn fix_brace_spacing(line: &mut String, message: &crate::messages::LintMessage) -> bool {
    use crate::messages::LintMessage;
    let fixed = match message {
        LintMessage::ExtraSpaceBeforeBracket => {
            BRACE_SPACE_BEFORE_RE.replace_all(line, "$1[").into_owned()
        }
        LintMessage::MissingSpaceBeforeOpenBrace => BRACE_MISSING_SPACE_RE
            .replace_all(line, "$1 {")
            .into_owned(),
        LintMessage::MissingSpaceBeforeElse => line.replace("}else", "} else"),
        _ => return false,
    };
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_paren_spacing(
    lines: &mut [String],
    idx: usize,
    message: &crate::messages::LintMessage,
) -> bool {
    use crate::messages::LintMessage;
    if idx >= lines.len() {
        return false;
    }
    match message {
        LintMessage::ExtraSpaceBeforeParenIn(_) | LintMessage::MissingSpaceBeforeOpenParen => {
            let fixed = lines[idx]
                .replace("if(", "if (")
                .replace("for(", "for (")
                .replace("while(", "while (")
                .replace("switch(", "switch (");
            if lines[idx] != fixed {
                lines[idx] = fixed;
                return true;
            }
        }
        LintMessage::MismatchingSpacesInsideParen => {
            let fixed = normalize_control_parentheses(&lines[idx]);
            if lines[idx] != fixed {
                lines[idx] = fixed;
                return true;
            }
        }
        LintMessage::ExtraSpaceBeforeParenInFuncCall => {
            let fixed = PAREN_SPACE_FUNC_CALL_BEFORE_RE
                .replace_all(&lines[idx], "$1(")
                .into_owned();
            if lines[idx] != fixed {
                lines[idx] = fixed;
                return true;
            }
        }
        LintMessage::ExtraSpaceAfterParen | LintMessage::ExtraSpaceAfterParenInFuncCall => {
            let fixed = PAREN_SPACE_AFTER_RE
                .replace_all(&lines[idx], "(")
                .into_owned();
            if lines[idx] != fixed {
                lines[idx] = fixed;
                return true;
            }
        }
        LintMessage::ExtraSpaceBeforeCloseParen => {
            let fixed = PAREN_SPACE_BEFORE_CLOSE_RE
                .replace_all(&lines[idx], ")")
                .into_owned();
            if lines[idx] != fixed {
                lines[idx] = fixed;
                return true;
            }
        }
        LintMessage::ClosingParenShouldBeMovedToPreviousLine if idx > 0 => {
            let Some(close_pos) = lines[idx].find(')') else {
                return false;
            };
            let before = lines[idx - 1].trim_end().to_string();
            let suffix = lines[idx][close_pos + 1..].trim_start().to_string();
            let new_prev = format!("{})", before);
            let indent = lines[idx]
                .chars()
                .take_while(|ch| ch.is_ascii_whitespace())
                .collect::<String>();
            let new_current = if suffix.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, suffix)
            };
            if lines[idx - 1] != new_prev || lines[idx] != new_current {
                lines[idx - 1] = new_prev;
                lines[idx] = new_current;
                return true;
            }
        }
        _ => {}
    }
    false
}

fn fix_operator_spacing(line: &mut String, message: &crate::messages::LintMessage) -> bool {
    use crate::messages::LintMessage;
    if let Some(parsed) = ParsedLine::parse(line) {
        let fixed = match message {
            LintMessage::MissingSpacesAround(op) => parsed
                .rewrite_code_segments(|code| add_spaces_around_operator(code, op.as_fix_str())),
            LintMessage::ExtraSpaceForOperator(op) => parsed.rewrite_code_segments(|code| {
                remove_spaces_after_unary_operator(code, op.as_fix_str())
            }),
            _ => None,
        };
        if let Some(fixed) = fixed
            && *line != fixed
        {
            *line = fixed;
            return true;
        }
    }

    let fixed = match message {
        LintMessage::MissingSpacesAround(op) => update_code(line, |code| {
            add_spaces_around_operator(code, op.as_fix_str())
        }),
        LintMessage::ExtraSpaceForOperator(op) => update_code(line, |code| {
            remove_spaces_after_unary_operator(code, op.as_fix_str())
        }),
        _ => return false,
    };
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_alt_tokens(line: &mut String) -> bool {
    if let Some(parsed) = ParsedLine::parse(line)
        && let Some(fixed) = parsed.rewrite_code_segments(|code| {
            let mut fixed = code.to_string();
            for (regex, replacement) in ALT_TOKEN_FIXES.iter() {
                fixed = regex.replace_all(&fixed, *replacement).into_owned();
            }
            fixed
        })
    {
        *line = fixed;
        return true;
    }

    let mut fixed = line.clone();
    for (regex, replacement) in ALT_TOKEN_FIXES.iter() {
        fixed = regex.replace_all(&fixed, *replacement).into_owned();
    }
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_check_macro(line: &mut String) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let Some(call) = parsed.find_call_expression(&[
        "DCHECK",
        "CHECK",
        "EXPECT_TRUE",
        "ASSERT_TRUE",
        "EXPECT_FALSE",
        "ASSERT_FALSE",
    ]) else {
        return false;
    };
    if call.arguments.len() != 1 {
        return false;
    }
    let check_macro =
        match check_macro_name_from_str(parsed.node_text(call.function).unwrap_or("").trim()) {
            Some(name) => name,
            None => return false,
        };
    let Some((lhs, op, rhs)) = parsed.binary_expression_parts(call.arguments[0]) else {
        return false;
    };
    let Some(op) = comparison_operator_from_str(op) else {
        return false;
    };
    let lhs_text = parsed.node_text(lhs).unwrap_or("").trim();
    let rhs_text = parsed.node_text(rhs).unwrap_or("").trim();
    if !is_check_const(lhs_text) && !is_check_const(rhs_text) {
        return false;
    }
    let Some(replacement) = replacement_check_macro(check_macro, op) else {
        return false;
    };
    let rebuilt = parsed.replace_node(
        call.node,
        &format!("{}({}, {})", replacement.as_str(), lhs_text, rhs_text),
    );
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_constructor_should_be_explicit(line: &mut String) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("explicit ") {
        return false;
    }
    let indent_len = line.len() - trimmed.len();
    let fixed = format!("{}explicit {}", &line[..indent_len], trimmed);
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_redundant_string_ctor(line: &mut String) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let Some(eq_pos) = line.find('=') else {
        return false;
    };
    if !line[..eq_pos].contains("string") {
        return false;
    }
    let Some(call) = parsed.find_call_expression(&["string"]) else {
        return false;
    };
    if call.node.start_byte() <= eq_pos || call.arguments.len() != 1 {
        return false;
    }
    let argument = parsed.node_text(call.arguments[0]).unwrap_or("").trim();
    if argument.is_empty() {
        return false;
    }
    let rebuilt = parsed.replace_node(call.node, argument);
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_deprecated_cast_style(line: &mut String, type_str: &str) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let normalized_type = normalize_cast_type(type_str);
    let Some(call) = parsed.find_call_expression_matching(|function_text, arguments| {
        normalize_cast_type(function_text) == normalized_type && arguments.len() == 1
    }) else {
        return false;
    };
    let argument = parsed.node_text(call.arguments[0]).unwrap_or("").trim();
    if argument.is_empty() {
        return false;
    }
    let rebuilt = parsed.replace_node(
        call.node,
        &format!("static_cast<{}>({})", type_str.trim(), argument),
    );
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_c_style_cast(line: &mut String, cast_type: &str, type_str: &str) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let normalized_type = normalize_cast_type(type_str);
    let Some(cast) = parsed.find_cast_expression_matching(|candidate_type, _value_node| {
        normalize_cast_type(candidate_type) == normalized_type
    }) else {
        return false;
    };
    let value = parsed.node_text(cast.value_node).unwrap_or("").trim();
    if value.is_empty() {
        return false;
    }
    let rebuilt = parsed.replace_node(
        cast.node,
        &format!("{}<{}>({})", cast_type.trim(), type_str.trim(), value),
    );
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_invalid_increment(line: &mut String) -> bool {
    if let Some(parsed) = ParsedLine::parse(line)
        && let Some(invalid) = parsed.find_invalid_increment_expression()
    {
        let operand = parsed.node_text(invalid.operand).unwrap_or("").trim();
        if !operand.is_empty() {
            let rebuilt =
                parsed.replace_node(invalid.node, &format!("(*{operand}){}", invalid.operator));
            if *line != rebuilt {
                *line = rebuilt;
                return true;
            }
        }
    }

    let fixed = update_code(line, |code| {
        let Some(captures) = POINTER_INCREMENT_FIX_RE.captures(code) else {
            return code.to_string();
        };
        let indent = captures.name("indent").map(|m| m.as_str()).unwrap_or("");
        let name = captures.name("name").map(|m| m.as_str()).unwrap_or("");
        let op = captures.name("op").map(|m| m.as_str()).unwrap_or("");
        let suffix = captures.name("suffix").map(|m| m.as_str()).unwrap_or("");
        format!("{indent}(*{name}){op}{suffix}")
    });
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_inheritance_redundancy(
    lines: &mut [String],
    idx: usize,
    message: &crate::messages::LintMessage,
) -> bool {
    use crate::messages::LintMessage;
    let Some(line) = lines.get(idx) else {
        return false;
    };
    let fixed = match message {
        LintMessage::RedundantVirtual => rewrite_inheritance_line(line, &INHERITANCE_VIRTUAL_RE),
        LintMessage::RedundantOverride => rewrite_inheritance_line(line, &INHERITANCE_OVERRIDE_RE),
        _ => return false,
    };
    if fixed.as_ref().is_some_and(|fixed| fixed != line) {
        lines[idx] = fixed.unwrap();
        return true;
    }

    if matches!(message, LintMessage::RedundantVirtual)
        && line.split("//").next().is_some_and(|code| {
            matches!(
                code.trim(),
                "override;" | "final;" | "override final;" | "final override;"
            )
        })
        && let Some(prev_idx) = previous_non_blank_line(lines, idx)
        && let Some(prev_fixed) =
            rewrite_inheritance_line(&lines[prev_idx], &INHERITANCE_VIRTUAL_RE)
        && prev_fixed != lines[prev_idx]
    {
        lines[prev_idx] = prev_fixed;
        return true;
    }

    false
}

fn rewrite_inheritance_line(line: &str, pattern: &Regex) -> Option<String> {
    if let Some(parsed) = ParsedLine::parse(line) {
        let fixed = parsed
            .rewrite_code_segments(|code| pattern.replace(code, "").into_owned())
            .unwrap_or_else(|| line.to_string());
        return (fixed != line).then_some(fixed);
    }
    let fixed = pattern.replace(line, "").into_owned();
    (fixed != line).then_some(fixed)
}

fn fix_endif_comment(line: &mut String) -> bool {
    let Some(captures) = ENDIF_TEXT_RE.captures(line) else {
        return false;
    };
    let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let suffix = captures.get(2).map(|m| m.as_str()).unwrap_or("").trim();
    let fixed = format!("{}  // {}", prefix, suffix);
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_make_pair(line: &mut String) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let Some(call) = parsed.find_call_expression(&["make_pair"]) else {
        return false;
    };
    let function_text = parsed.node_text(call.function).unwrap_or("").trim();
    let Some(rewritten_function) = strip_template_arguments(function_text) else {
        return false;
    };
    let rebuilt = parsed.replace_node(call.function, &rewritten_function);
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_memset(line: &mut String) -> bool {
    if let Some(parsed) = ParsedLine::parse(line)
        && let Some(call) = parsed.find_call_expression(&["memset"])
        && call.arguments.len() == 3
    {
        let function_text = parsed.node_text(call.function).unwrap_or("").trim();
        let target = parsed.node_text(call.arguments[0]).unwrap_or("").trim();
        let size = parsed.node_text(call.arguments[1]).unwrap_or("").trim();
        let zero = parsed.node_text(call.arguments[2]).unwrap_or("").trim();
        if zero == "0" {
            let rebuilt = parsed.replace_node(
                call.node,
                &format!("{}({}, 0, {})", function_text, target, size),
            );
            if *line != rebuilt {
                *line = rebuilt;
                return true;
            }
        }
    }

    let fixed = MEMSET_FIX_RE
        .replace(line, "memset($1, 0, $2)")
        .into_owned();
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_printf_format(line: &mut String, message: &crate::messages::LintMessage) -> bool {
    use crate::messages::LintMessage;
    if let Some(parsed) = ParsedLine::parse(line) {
        let fixed = match message {
            LintMessage::PrintfFormatDeprecatedQ => parsed
                .find_call_expression(&[
                    "printf",
                    "fprintf",
                    "sprintf",
                    "snprintf",
                    "vprintf",
                    "vfprintf",
                    "vsprintf",
                    "vsnprintf",
                    "asprintf",
                    "vasprintf",
                ])
                .and_then(|call| {
                    parsed.rewrite_string_literals_in(call.node.byte_range(), |literal| {
                        Some(PRINTF_Q_RE.replace_all(literal, "%${1}ll").into_owned())
                    })
                }),
            LintMessage::PrintfFormatUndefinedEscape => parsed.rewrite_string_literals(|literal| {
                Some(
                    literal
                        .replace(r"\%", "%")
                        .replace(r"\[", "[")
                        .replace(r"\(", "(")
                        .replace(r"\{", "{"),
                )
            }),
            _ => None,
        };
        if let Some(fixed) = fixed
            && *line != fixed
        {
            *line = fixed;
            return true;
        }
    }

    let fixed = match message {
        LintMessage::PrintfFormatDeprecatedQ => {
            PRINTF_Q_RE.replace_all(line, "%${1}ll").into_owned()
        }
        LintMessage::PrintfFormatUndefinedEscape => line
            .replace(r"\%", "%")
            .replace(r"\[", "[")
            .replace(r"\(", "(")
            .replace(r"\{", "{"),
        _ => return false,
    };
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_namespace_indentation(line: &mut String) -> bool {
    let fixed = line
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace())
        .to_string();
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn build_facts<'a>(
    arena: &'a Bump,
    path: &Path,
    options: &Options,
    lines: &[String],
) -> (CleansedLines<'a>, FileFacts<'a>) {
    let filename = path.to_string_lossy();
    let mut arena_lines = bumpalo::collections::Vec::with_capacity_in(lines.len(), arena);
    for line in lines {
        arena_lines.push(arena.alloc_str(line) as &str);
    }
    let arena_lines = arena_lines.into_bump_slice();

    let clean_lines = CleansedLines::new_with_options(arena, arena_lines, options, &filename);
    let facts = FileFacts::new(&clean_lines, arena);
    (clean_lines, facts)
}

struct FactsCache<'a> {
    arena: *mut Bump,
    cached_fingerprint: Option<u64>,
    cached: Option<(CleansedLines<'a>, FileFacts<'a>)>,
    _marker: PhantomData<&'a mut Bump>,
}

impl<'a> FactsCache<'a> {
    fn new(arena: &'a mut Bump) -> Self {
        arena.reset();
        Self {
            arena,
            cached_fingerprint: None,
            cached: None,
            _marker: PhantomData,
        }
    }

    fn get(
        &mut self,
        path: &Path,
        options: &Options,
        lines: &[String],
    ) -> (&CleansedLines<'a>, &FileFacts<'a>) {
        let fingerprint = fingerprint_lines(lines);
        if self.cached_fingerprint != Some(fingerprint) {
            self.cached = None;
            // SAFETY: arena points to the thread-local bump arena borrowed for the whole
            // apply_line_fixes call; the cache drops old references before reset/rebuild.
            let arena = unsafe { &mut *self.arena };
            arena.reset();
            self.cached = Some(build_facts(arena, path, options, lines));
            self.cached_fingerprint = Some(fingerprint);
        }
        let (clean_lines, facts) = self
            .cached
            .as_ref()
            .expect("facts cache should be initialized");
        (clean_lines, facts)
    }
}

fn fingerprint_lines(lines: &[String]) -> u64 {
    let mut hasher = FxHasher::default();
    lines.len().hash(&mut hasher);
    for line in lines {
        line.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
fn fix_access_specifier_indentation(
    path: &Path,
    options: &Options,
    lines: &mut [String],
    idx: usize,
) -> bool {
    FIXER_ARENA.with(|arena_cell| {
        // SAFETY: The arena is thread-local and accessed synchronously in this helper.
        let arena = unsafe { &mut *arena_cell.get() };
        arena.reset();
        let (clean_lines, facts) = build_facts(arena, path, options, lines);
        fix_access_specifier_indentation_with_facts(lines, idx, &clean_lines, &facts)
    })
}

fn fix_access_specifier_indentation_with_facts(
    lines: &mut [String],
    idx: usize,
    clean_lines: &CleansedLines<'_>,
    facts: &FileFacts<'_>,
) -> bool {
    let Some(class_range) = facts.enclosing_class_range(idx) else {
        return false;
    };
    let class_indent =
        line_utils::get_indent_level(clean_lines.lines_without_raw_strings[class_range.start]);
    let Some(captures) = ACCESS_SPECIFIER_FIX_RE.captures(&lines[idx]) else {
        return false;
    };
    let access = captures.name("access").map(|m| m.as_str()).unwrap_or("");
    let slots = captures.name("slots").map(|m| m.as_str()).unwrap_or("");
    let suffix = captures.name("suffix").map(|m| m.as_str()).unwrap_or("");
    let fixed = format!(
        "{}{}{}:{}",
        " ".repeat(class_indent + 1),
        access,
        slots,
        suffix
    );
    if lines[idx] != fixed {
        lines[idx] = fixed;
        return true;
    }
    false
}

#[cfg(test)]
fn fix_class_closing_brace_alignment(
    path: &Path,
    options: &Options,
    lines: &mut [String],
    idx: usize,
) -> bool {
    FIXER_ARENA.with(|arena_cell| {
        // SAFETY: The arena is thread-local and accessed synchronously in this helper.
        let arena = unsafe { &mut *arena_cell.get() };
        arena.reset();
        let (clean_lines, facts) = build_facts(arena, path, options, lines);
        fix_class_closing_brace_alignment_with_facts(lines, idx, &clean_lines, &facts)
    })
}

fn fix_class_closing_brace_alignment_with_facts(
    lines: &mut [String],
    idx: usize,
    clean_lines: &CleansedLines<'_>,
    facts: &FileFacts<'_>,
) -> bool {
    let Some(class_range) = facts.enclosing_class_range(idx) else {
        return false;
    };
    let class_indent =
        line_utils::get_indent_level(clean_lines.lines_without_raw_strings[class_range.start]);
    let trimmed = lines[idx].trim_start();
    if !trimmed.starts_with('}') {
        return false;
    }
    let suffix = &trimmed[1..];
    let fixed = format!("{}}}{}", " ".repeat(class_indent), suffix);
    if lines[idx] != fixed {
        lines[idx] = fixed;
        return true;
    }
    false
}

fn fix_storage_class(line: &mut String) -> bool {
    if let Some(parsed) = ParsedLine::parse(line)
        && let Some(fixed) = parsed.rewrite_code_segments(rewrite_storage_class_segment)
    {
        *line = fixed;
        return true;
    }

    let fixed = update_code(line, rewrite_storage_class_segment);
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_vlog_macro(line: &mut String) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let Some(call) = parsed.find_call_expression(&["VLOG"]) else {
        return false;
    };
    if call.arguments.len() != 1 {
        return false;
    }
    let level = parsed.node_text(call.arguments[0]).unwrap_or("").trim();
    if !matches!(level, "INFO" | "ERROR" | "WARNING" | "DFATAL" | "FATAL") {
        return false;
    }
    let rebuilt = parsed.replace_node(call.function, "LOG");
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_global_const_string(
    line: &mut String,
    prefix: &str,
    suffix_const: &str,
    name: &str,
) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let code_end = parsed.first_comment_start().unwrap_or(line.len());
    if let Some(captures) = GLOBAL_CONST_STRING_FIX_RE.captures(line) {
        let indent = captures.name("indent").map(|m| m.as_str()).unwrap_or("");
        let captured_name = captures.name("name").map(|m| m.as_str()).unwrap_or("");
        if captured_name != name {
            return false;
        }
        let Some(eq_pos) = line[..code_end].find('=') else {
            return false;
        };
        let rhs_end = line[..code_end].rfind(';').unwrap_or(code_end);
        if eq_pos >= rhs_end || !parsed.rhs_contains_only_string_literals(eq_pos + 1, rhs_end) {
            return false;
        }
        let rest = captures.name("rest").map(|m| m.as_str()).unwrap_or("");
        let fixed = format!("{indent}{prefix}char{suffix_const} {name}[]{rest}");
        if *line != fixed {
            *line = fixed;
            return true;
        }
        return false;
    }

    let Some(captures) = GLOBAL_CONST_STRING_DIRECT_INIT_RE.captures(line) else {
        return false;
    };
    let indent = captures.name("indent").map(|m| m.as_str()).unwrap_or("");
    let captured_name = captures.name("name").map(|m| m.as_str()).unwrap_or("");
    if captured_name != name {
        return false;
    }
    let open = captures.name("open").map(|m| m.as_str()).unwrap_or("");
    let close = captures.name("close").map(|m| m.as_str()).unwrap_or("");
    if !matches!((open, close), ("(", ")") | ("{", "}")) {
        return false;
    }
    let init = captures.name("init").map(|m| m.as_str()).unwrap_or("");
    let init_range = captures.name("init").map(|m| m.range()).unwrap_or(0..0);
    if init_range.is_empty()
        || !parsed.rhs_contains_only_string_literals(init_range.start, init_range.end)
    {
        return false;
    }
    let suffix = captures.name("suffix").map(|m| m.as_str()).unwrap_or("");
    let fixed = format!("{indent}{prefix}char{suffix_const} {name}[] = {init}{suffix}");
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn fix_snprintf_sizeof(line: &mut String, buffer: &str, size: &str) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let normalized_buffer = buffer.trim();
    let normalized_size = size.trim();
    let Some(call) = parsed.find_call_expression_matching(|function_text, arguments| {
        base_name(function_text) == "snprintf"
            && arguments.len() >= 2
            && parsed
                .node_text(arguments[0])
                .is_some_and(|text| text.trim() == normalized_buffer)
            && parsed
                .node_text(arguments[1])
                .is_some_and(|text| text.trim() == normalized_size)
    }) else {
        return false;
    };
    let rebuilt = parsed.replace_node(call.arguments[1], &format!("sizeof({normalized_buffer})"));
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn fix_potential_format_string_bug(line: &mut String, function: &str, arg: &str) -> bool {
    let Some(parsed) = ParsedLine::parse(line) else {
        return false;
    };
    let normalized_function = function.trim();
    let normalized_arg = arg.trim();
    let Some(call) = parsed.find_call_expression_matching(|function_text, arguments| {
        base_name(function_text) == normalized_function
            && arguments.len() == 1
            && parsed
                .node_text(arguments[0])
                .is_some_and(|text| text.trim() == normalized_arg)
    }) else {
        return false;
    };
    let function_text = parsed.node_text(call.function).unwrap_or("").trim();
    let arg_text = parsed.node_text(call.arguments[0]).unwrap_or("").trim();
    if function_text.is_empty() || arg_text.is_empty() {
        return false;
    }
    let rebuilt = parsed.replace_node(call.node, &format!("{function_text}(\"%s\", {arg_text})"));
    if *line != rebuilt {
        *line = rebuilt;
        return true;
    }
    false
}

fn rewrite_storage_class_segment(code: &str) -> String {
    let Some(captures) = STORAGE_CLASS_FIX_RE.captures(code) else {
        return code.to_string();
    };
    let prefix = captures
        .name("prefix")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim();
    if prefix.is_empty() {
        return code.to_string();
    }
    let indent = captures.name("indent").map(|m| m.as_str()).unwrap_or("");
    let storage = captures.name("storage").map(|m| m.as_str()).unwrap_or("");
    let suffix = captures
        .name("suffix")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim();
    format!("{}{} {} {}", indent, storage, prefix, suffix)
}

fn strip_template_arguments(function_text: &str) -> Option<String> {
    let start = function_text.find('<')?;
    let mut depth = 0usize;
    let mut end = None;
    for (idx, ch) in function_text.char_indices().skip(start) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let mut rebuilt = String::with_capacity(function_text.len());
    rebuilt.push_str(function_text[..start].trim_end());
    rebuilt.push_str(&function_text[end + 1..]);
    Some(rebuilt)
}

fn check_macro_name_from_str(value: &str) -> Option<crate::messages::CheckMacroName> {
    Some(match value.rsplit("::").next().unwrap_or(value).trim() {
        "DCHECK" => crate::messages::CheckMacroName::Dcheck,
        "CHECK" => crate::messages::CheckMacroName::Check,
        "EXPECT_TRUE" => crate::messages::CheckMacroName::ExpectTrue,
        "ASSERT_TRUE" => crate::messages::CheckMacroName::AssertTrue,
        "EXPECT_FALSE" => crate::messages::CheckMacroName::ExpectFalse,
        "ASSERT_FALSE" => crate::messages::CheckMacroName::AssertFalse,
        _ => return None,
    })
}

fn comparison_operator_from_str(value: &str) -> Option<crate::messages::ComparisonOperator> {
    Some(match value {
        "==" => crate::messages::ComparisonOperator::Eq,
        "!=" => crate::messages::ComparisonOperator::Ne,
        ">=" => crate::messages::ComparisonOperator::Ge,
        ">" => crate::messages::ComparisonOperator::Gt,
        "<=" => crate::messages::ComparisonOperator::Le,
        "<" => crate::messages::ComparisonOperator::Lt,
        _ => return None,
    })
}

fn normalize_cast_type(value: &str) -> String {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(trimmed);
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn update_code(line: &str, transform: impl FnOnce(&str) -> String) -> String {
    if let Some(captures) = COMMENT_SPLIT_RE.captures(line) {
        let code = captures.name("code").map(|m| m.as_str()).unwrap_or("");
        let comment = captures.name("comment").map(|m| m.as_str()).unwrap_or("");
        format!("{}{}", transform(code), comment)
    } else {
        transform(line)
    }
}

fn update_code_and_comment(line: &mut String, transform: impl FnOnce(&str) -> String) -> bool {
    let fixed = update_code(line, transform);
    if *line != fixed {
        *line = fixed;
        return true;
    }
    false
}

fn add_spaces_around_operator(code: &str, op: &str) -> String {
    OPERATOR_SPACE_REGEX_CACHE.with_borrow_mut(|cache| {
        let pattern = cache.entry(op.to_string()).or_insert_with(|| {
            Regex::new(&format!(
                r#"(?P<lhs>\S)\s*{}\s*(?P<rhs>\S)"#,
                regex::escape(op)
            ))
            .unwrap()
        });
        pattern
            .replace_all(code, format!("$lhs {} $rhs", op))
            .into_owned()
    })
}

fn remove_spaces_after_unary_operator(code: &str, op: &str) -> String {
    match op.trim() {
        "!" => UNARY_NOT_SPACE_RE.replace_all(code, "!").into_owned(),
        "~" => UNARY_COMPL_SPACE_RE.replace_all(code, "~").into_owned(),
        _ => code.to_string(),
    }
}

fn normalize_control_parentheses(line: &str) -> String {
    let mut fixed = line
        .replace("if(", "if (")
        .replace("for(", "for (")
        .replace("while(", "while (")
        .replace("switch(", "switch (");
    fixed = PAREN_SPACE_AFTER_RE.replace_all(&fixed, "(").into_owned();
    PAREN_SPACE_BEFORE_CLOSE_RE
        .replace_all(&fixed, ")")
        .into_owned()
}

fn find_line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    while index + 1 < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        if in_string {
            if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if in_char {
            if byte == b'\\' {
                escaped = true;
            } else if byte == b'\'' {
                in_char = false;
            }
            index += 1;
            continue;
        }

        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'\'' {
            in_char = true;
            index += 1;
            continue;
        }
        if byte == b'/' && bytes[index + 1] == b'/' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn previous_non_blank_line(lines: &[String], start: usize) -> Option<usize> {
    (0..start).rev().find(|&idx| !lines[idx].trim().is_empty())
}

fn find_matching_paren(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in line.char_indices().skip(open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
fn find_matching_angle_bracket(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in line.char_indices().skip(open) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
fn split_comparison_expression(
    expression: &str,
) -> Option<(&str, crate::messages::ComparisonOperator, &str)> {
    let mut depth = 0usize;
    let bytes = expression.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 {
            for (op, len) in [
                (crate::messages::ComparisonOperator::Eq, 2usize),
                (crate::messages::ComparisonOperator::Ne, 2usize),
                (crate::messages::ComparisonOperator::Ge, 2usize),
                (crate::messages::ComparisonOperator::Le, 2usize),
                (crate::messages::ComparisonOperator::Gt, 1usize),
                (crate::messages::ComparisonOperator::Lt, 1usize),
            ] {
                if expression[index..].starts_with(op.as_str()) {
                    return Some((&expression[..index], op, &expression[index + len..]));
                }
            }
        }
        index += 1;
    }
    None
}

fn is_check_const(value: &str) -> bool {
    let value = value.trim();
    (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
}

fn replacement_check_macro(
    check_macro: crate::messages::CheckMacroName,
    op: crate::messages::ComparisonOperator,
) -> Option<crate::messages::CheckMacroReplacement> {
    use crate::messages::{
        CheckMacroName as M, CheckMacroReplacement as R, ComparisonOperator as O,
    };
    Some(match (check_macro, op) {
        (M::Dcheck, O::Eq) => R::DcheckEq,
        (M::Dcheck, O::Ne) => R::DcheckNe,
        (M::Dcheck, O::Ge) => R::DcheckGe,
        (M::Dcheck, O::Gt) => R::DcheckGt,
        (M::Dcheck, O::Le) => R::DcheckLe,
        (M::Dcheck, O::Lt) => R::DcheckLt,
        (M::Check, O::Eq) => R::CheckEq,
        (M::Check, O::Ne) => R::CheckNe,
        (M::Check, O::Ge) => R::CheckGe,
        (M::Check, O::Gt) => R::CheckGt,
        (M::Check, O::Le) => R::CheckLe,
        (M::Check, O::Lt) => R::CheckLt,
        (M::ExpectTrue, O::Eq) => R::ExpectEq,
        (M::ExpectTrue, O::Ne) => R::ExpectNe,
        (M::ExpectTrue, O::Ge) => R::ExpectGe,
        (M::ExpectTrue, O::Gt) => R::ExpectGt,
        (M::ExpectTrue, O::Le) => R::ExpectLe,
        (M::ExpectTrue, O::Lt) => R::ExpectLt,
        (M::AssertTrue, O::Eq) => R::AssertEq,
        (M::AssertTrue, O::Ne) => R::AssertNe,
        (M::AssertTrue, O::Ge) => R::AssertGe,
        (M::AssertTrue, O::Gt) => R::AssertGt,
        (M::AssertTrue, O::Le) => R::AssertLe,
        (M::AssertTrue, O::Lt) => R::AssertLt,
        (M::ExpectFalse, O::Eq) => R::ExpectNe,
        (M::ExpectFalse, O::Ne) => R::ExpectEq,
        (M::ExpectFalse, O::Ge) => R::ExpectLt,
        (M::ExpectFalse, O::Gt) => R::ExpectLe,
        (M::ExpectFalse, O::Le) => R::ExpectGt,
        (M::ExpectFalse, O::Lt) => R::ExpectGe,
        (M::AssertFalse, O::Eq) => R::AssertNe,
        (M::AssertFalse, O::Ne) => R::AssertEq,
        (M::AssertFalse, O::Ge) => R::AssertLt,
        (M::AssertFalse, O::Gt) => R::AssertLe,
        (M::AssertFalse, O::Le) => R::AssertGt,
        (M::AssertFalse, O::Lt) => R::AssertGe,
    })
}

#[derive(Debug, Clone)]
struct IncludeEntry {
    include: String,
    raw_line: String,
    kind: IncludeKind,
    alpha_key: String,
}

fn include_kind_rank(kind: IncludeKind) -> usize {
    match kind {
        IncludeKind::LikelyMyHeader | IncludeKind::PossibleMyHeader => 0,
        IncludeKind::CSystem => 1,
        IncludeKind::CppSystem => 2,
        IncludeKind::OtherSystem => 3,
        IncludeKind::OtherHeader => 4,
    }
}

fn canonicalize_alpha(include: &str) -> String {
    include
        .replace("-inl.h", ".h")
        .replace('-', "_")
        .to_ascii_lowercase()
}

fn classify_include(
    path_from_repo: &Path,
    include: &Path,
    used_angle_brackets: bool,
    include_order: IncludeOrder,
) -> IncludeKind {
    let include_str = include.to_string_lossy().replace('\\', "/");
    let is_cpp_header = c_headers::CPP_HEADERS
        .binary_search(&include_str.as_str())
        .is_ok();
    let include_ext = include
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext))
        .unwrap_or_default();
    let is_system =
        used_angle_brackets && !matches!(include_ext.as_str(), ".hh" | ".hpp" | ".hxx" | ".h++");
    let is_std_c_header = include_order == IncludeOrder::Default
        || c_headers::C_HEADERS
            .binary_search(&include_str.as_str())
            .is_ok();

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

fn expected_header_guard(path: &Path, options: &Options) -> String {
    generate_guard(&relative_from_subdir(
        &relative_from_repository(path, &options.repository),
        &options.root,
    ))
}

fn generate_guard(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        if let Some(part) = component.as_os_str().to_str()
            && !part.is_empty()
            && part != "."
        {
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

fn relative_from_repository(file: &Path, repository: &Path) -> PathBuf {
    if file == Path::new("-") {
        return PathBuf::from("-");
    }
    if !repository.as_os_str().is_empty()
        && let (Ok(file_abs), Ok(repo_abs)) = (
            std::fs::canonicalize(file),
            std::fs::canonicalize(repository),
        )
        && let Ok(relative) = file_abs.strip_prefix(repo_abs)
    {
        return relative.to_path_buf();
    }

    let Ok(file_abs) = std::fs::canonicalize(file) else {
        return file.to_path_buf();
    };
    let mut current = file_abs.parent().unwrap_or(file_abs.as_path());
    let mut project_root = current.to_path_buf();
    loop {
        if current.join(".git").exists()
            || current.join(".hg").exists()
            || current.join(".svn").exists()
        {
            project_root = current.to_path_buf();
            break;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    file_abs
        .strip_prefix(project_root)
        .map(Path::to_path_buf)
        .unwrap_or(file_abs)
}

fn relative_from_subdir(file: &Path, subdir: &Path) -> PathBuf {
    if subdir.as_os_str().is_empty() {
        return file.to_path_buf();
    }
    if let Ok(relative) = file.strip_prefix(subdir) {
        return relative.to_path_buf();
    }
    if let (Ok(file_abs), Ok(subdir_abs)) =
        (std::fs::canonicalize(file), std::fs::canonicalize(subdir))
        && let Ok(relative) = file_abs.strip_prefix(subdir_abs)
    {
        return relative.to_path_buf();
    }
    file.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(unix)]
    use std::{fs, os::unix::fs::PermissionsExt};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("cpplint-rs-fixer-{}-{}", unique, counter))
    }

    #[test]
    fn fix_file_rewrites_common_fixable_rules() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.h");
        std::fs::write(
            &file,
            "// Copyright 2026\n#include <string>\n#include <stdio.h>\nint x=0; //comment\n",
        )
        .unwrap();

        let mut options = Options::new();
        options.add_filter("+build/include_alpha");
        assert!(fix_file_in_place(&file, &options).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("#ifndef SAMPLE_H_"));
        assert!(contents.contains("#include <stdio.h>\n#include <string>"));
        assert!(contents.contains("int x = 0;  // comment"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_adds_final_newline_and_normalizes_crlf() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(&file, b"int x=0;\r\nint y=1;\n").unwrap();

        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        assert!(fix_file_in_place(&file, &options).unwrap());

        let bytes = std::fs::read(&file).unwrap();
        assert!(!bytes.windows(2).any(|pair| pair == b"\r\n"));
        assert!(String::from_utf8_lossy(&bytes).ends_with('\n'));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_handles_readability_and_runtime_rewrites() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(
            &file,
            concat!(
                "// Copyright 2026\n",
                "#include <cstdio>\n",
                "#include <cstring>\n",
                "#include <utility>\n",
                "\n",
                "class Demo {\n",
                " public:\n",
                "  virtual void Run() override;\n",
                "  virtual void Split()\n",
                "      override;  // virtual comment stays\n",
                "  Demo(int value);\n",
                "};\n",
                "\n",
                "namespace foo {\n",
                "int a0;\n",
                "int a1;\n",
                "int a2;\n",
                "int a3;\n",
                "int a4;\n",
                "int a5;\n",
                "int a6;\n",
                "int a7;\n",
                "int a8;\n",
                "int a9;\n",
                "}\n",
                "\n",
                "void f(char* buf, int size, char kind, int value) {\n",
                "  auto pair = make_pair<int, int>(1,2);\n",
                "  memset(buf, size, 0);\n",
                "  VLOG(INFO) << value;\n",
                "  CHECK(kind == 'x');\n",
                "  std::string text = std::string(\"cpplint\");\n",
                "  long cast_a = int(value);\n",
                "  auto cast_b = (int)value;\n",
                "  snprintf(buf, 32, \"%s\", value);\n",
                "  printf(text);\n",
                "  printf(\"%q\", value);\n",
                "  printf(\"\\%\", value);\n",
                "}\n",
            ),
        )
        .unwrap();

        let mut options = Options::new();
        options.add_filter("-build/include_what_you_use");
        assert!(fix_file_in_place(&file, &options).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("void Run() override;"));
        assert!(!contents.contains("virtual void Run() override;"));
        assert!(contents.contains("void Split()"));
        assert!(contents.contains("virtual comment stays"));
        assert!(!contents.contains("virtual void Split()"));
        assert!(contents.contains("explicit Demo(int value);"));
        assert!(contents.contains("auto pair = make_pair(1, 2);"));
        assert!(contents.contains("memset(buf, 0, size);"));
        assert!(contents.contains("LOG(INFO) << value;"));
        assert!(contents.contains("CHECK_EQ(kind, 'x');"));
        assert!(contents.contains("std::string text = \"cpplint\";"));
        assert!(contents.contains("long cast_a = static_cast<int>(value);"));
        assert!(contents.contains("auto cast_b = static_cast<int>(value);"));
        assert!(contents.contains("snprintf(buf, sizeof(buf), \"%s\", value);"));
        assert!(contents.contains("printf(\"%s\", text);"));
        assert!(contents.contains("printf(\"%ll\", value);"));
        assert!(contents.contains("printf(\"%\", value);"));
        assert!(contents.contains("}  // namespace foo"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_uses_tree_sitter_to_limit_runtime_and_macro_rewrites() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(
            &file,
            concat!(
                "// Copyright 2026\n",
                "#include <cstdio>\n",
                "#include <string>\n",
                "#include <utility>\n",
                "\n",
                "static const std::string kName = \"hello\";\n",
                "static const std::string kJoined = \"hel\" \"lo\";\n",
                "static const std::string kCtor(\"world\");\n",
                "static const std::string kFactory = BuildLabel(\"hello\");\n",
                "\n",
                "void f(bool lhs, bool rhs, char kind) {\n",
                "  std::string text = std::string(\"value\");\n",
                "  *count++;\n",
                "  bool alt = lhs and rhs; const char* token = \"and\";\n",
                "  auto pair = std::make_pair<int, int>(1, 2);  // std::make_pair<int, int>(3, 4)\n",
                "  CHECK(kind == 'x') << \"CHECK(kind == 'x')\";\n",
                "  VLOG(INFO) << \"VLOG(INFO)\";\n",
                "  StringPrintf(text);\n",
                "  const char* format = \"%q \\\\%\"; printf(\"%q \\\\%\", kind);\n",
                "}\n",
            ),
        )
        .unwrap();

        let options = Options::new();
        assert!(fix_file_in_place(&file, &options).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("static const char kName[] = \"hello\";"));
        assert!(contents.contains("static const char kJoined[] = \"hel\" \"lo\";"));
        assert!(contents.contains("static const char kCtor[] = \"world\";"));
        assert!(contents.contains("static const std::string kFactory = BuildLabel(\"hello\");"));
        assert!(contents.contains("std::string text = \"value\";"));
        assert!(contents.contains("(*count)++;"));
        assert!(contents.contains("bool alt = lhs && rhs; const char* token = \"and\";"));
        assert!(
            contents
                .contains("auto pair = std::make_pair(1, 2);  // std::make_pair<int, int>(3, 4)")
        );
        assert!(contents.contains("CHECK_EQ(kind, 'x') << \"CHECK(kind == 'x')\";"));
        assert!(contents.contains("LOG(INFO) << \"VLOG(INFO)\";"));
        assert!(contents.contains("StringPrintf(\"%s\", text);"));
        assert!(
            contents.contains("const char* format = \"%q \\\\%\"; printf(\"%ll \\\\%\", kind);")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_adds_iwyu_headers_and_rewrites_empty_bodies() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(
            &file,
            concat!(
                "// Copyright 2026\n",
                "void f() {\n",
                "  std::string name;\n",
                "  if (true);\n",
                "  while (ready);\n",
                "  for (;;);\n",
                "  if (flag,\n",
                "      check()) {\n",
                "  }\n",
                "}\n",
            ),
        )
        .unwrap();

        let options = Options::new();
        assert!(fix_file_in_place(&file, &options).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("// Copyright 2026\n#include <string>\n"));
        assert!(contents.contains("if (true) {}"));
        assert!(contents.contains("while (ready) {}"));
        assert!(contents.contains("for (;;) {}"));
        assert!(contents.contains("check()) {}"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_handles_layout_namespace_and_indent_rewrites() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(
            &file,
            concat!(
                "// Copyright 2026\n",
                "namespace demo {\n",
                "    int value = 0;\n",
                "class Foo {\n",
                "public:\n",
                "\n",
                "  int static number;\n",
                "  void Run() {\n",
                "    if (ready) {\n",
                "\n",
                "      Work();\n",
                "\n",
                "    }\n",
                "  }\n",
                " };\n",
                "}\n",
            ),
        )
        .unwrap();

        let options = Options::new();
        assert!(fix_file_in_place(&file, &options).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("namespace demo {\nint value = 0;\nclass Foo {"));
        assert!(contents.contains("class Foo {\n public:\n  static int number;"));
        assert!(!contents.contains("public:\n\n"));
        assert!(contents.contains("if (ready) {\n      Work();\n    }"));
        assert!(contents.contains("\n};\n"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn helper_fixers_cover_common_string_rewrites() {
        let mut line = String::from("int x; //comment");
        assert!(fix_comment_spacing(
            &mut line,
            &crate::messages::LintMessage::AtLeastTwoSpacesBetweenCodeAndComments
        ));
        assert_eq!(line, "int x;  //comment");

        let mut line = String::from("//TODO(user):fix this");
        assert!(fix_todo_spacing(&mut line));
        assert_eq!(line, "// TODO(user): fix this");

        let mut line = String::from("if (ready);");
        assert!(fix_empty_control_body(&mut line, &["if"]));
        assert_eq!(line, "if (ready) {}");

        let mut line = String::from("for (auto value:collection) {}");
        assert!(fix_range_for_colon(&mut line));
        assert!(line.contains(" : "));

        let mut line = String::from("int x [10];");
        assert!(fix_brace_spacing(
            &mut line,
            &crate::messages::LintMessage::ExtraSpaceBeforeBracket
        ));
        assert_eq!(line, "int x[10];");

        let mut line = String::from("if (ready){");
        assert!(fix_brace_spacing(
            &mut line,
            &crate::messages::LintMessage::MissingSpaceBeforeOpenBrace
        ));
        assert_eq!(line, "if (ready) {");

        let mut line = String::from("}else");
        assert!(fix_brace_spacing(
            &mut line,
            &crate::messages::LintMessage::MissingSpaceBeforeElse
        ));
        assert_eq!(line, "} else");

        let mut lines = vec![String::from("if(ready)")];
        assert!(fix_paren_spacing(
            &mut lines,
            0,
            &crate::messages::LintMessage::MissingSpaceBeforeOpenParen
        ));
        assert_eq!(lines[0], "if (ready)");

        let mut lines = vec![String::from("foo( bar )")];
        assert!(fix_paren_spacing(
            &mut lines,
            0,
            &crate::messages::LintMessage::MismatchingSpacesInsideParen
        ));
        assert_eq!(lines[0], "foo(bar)");

        let mut line = String::from("x = 1 and y or z");
        assert!(fix_alt_tokens(&mut line));
        assert_eq!(line, "x = 1 && y || z");

        let mut line = String::from("bool ok = left!=right; const char* cmp = \"!=\";");
        assert!(fix_operator_spacing(
            &mut line,
            &crate::messages::LintMessage::MissingSpacesAround(crate::messages::OperatorSymbol::Ne)
        ));
        assert_eq!(line, "bool ok = left != right; const char* cmp = \"!=\";");

        let mut line = String::from("CHECK(value == \"kFoo\")");
        assert!(fix_check_macro(&mut line));
        assert!(line.contains("CHECK_EQ"));

        let mut line = String::from("std::string name = std::string(\"cpplint\");");
        assert!(fix_redundant_string_ctor(&mut line));
        assert_eq!(line, "std::string name = \"cpplint\";");

        let mut line = String::from("std::string name = std::string(prefix, suffix);");
        assert!(!fix_redundant_string_ctor(&mut line));

        let mut line = String::from("  Foo(int value);");
        assert!(fix_constructor_should_be_explicit(&mut line));
        assert_eq!(line, "  explicit Foo(int value);");

        let mut line = String::from("long cast_a = int(value);");
        assert!(fix_deprecated_cast_style(&mut line, "int"));
        assert_eq!(line, "long cast_a = static_cast<int>(value);");

        let mut line = String::from("auto cast_b = (int)value;");
        assert!(fix_c_style_cast(&mut line, "static_cast", "int"));
        assert_eq!(line, "auto cast_b = static_cast<int>(value);");

        let mut line = String::from("char* text = (char*)\"hi\";");
        assert!(fix_c_style_cast(&mut line, "const_cast", "char*"));
        assert_eq!(line, "char* text = const_cast<char*>(\"hi\");");

        let mut line = String::from("*count++;");
        assert!(fix_invalid_increment(&mut line));
        assert_eq!(line, "(*count)++;");

        let mut line = String::from("*items[i]--;");
        assert!(fix_invalid_increment(&mut line));
        assert_eq!(line, "(*items[i])--;");

        let mut line = String::from("snprintf(buf, 32, \"%s\", value);");
        assert!(fix_snprintf_sizeof(&mut line, "buf", "32"));
        assert_eq!(line, "snprintf(buf, sizeof(buf), \"%s\", value);");

        let mut line = String::from("printf(text);");
        assert!(fix_potential_format_string_bug(&mut line, "printf", "text"));
        assert_eq!(line, "printf(\"%s\", text);");

        let mut line = String::from("StringPrintf(text);");
        assert!(fix_potential_format_string_bug(
            &mut line,
            "StringPrintf",
            "text"
        ));
        assert_eq!(line, "StringPrintf(\"%s\", text);");

        let mut lines = vec![String::from("virtual void Run() override;")];
        assert!(fix_inheritance_redundancy(
            &mut lines,
            0,
            &crate::messages::LintMessage::RedundantVirtual
        ));
        assert_eq!(lines[0], "void Run() override;");

        let mut lines = vec![String::from(
            "virtual void Run() override; // virtual comment stays",
        )];
        assert!(fix_inheritance_redundancy(
            &mut lines,
            0,
            &crate::messages::LintMessage::RedundantVirtual
        ));
        assert_eq!(lines[0], "void Run() override; // virtual comment stays");

        let mut lines = vec![
            String::from("virtual void Run()"),
            String::from("override;"),
        ];
        assert!(fix_inheritance_redundancy(
            &mut lines,
            1,
            &crate::messages::LintMessage::RedundantVirtual
        ));
        assert_eq!(lines[0], "void Run()");
        assert_eq!(lines[1], "override;");

        let mut line = String::from("#endif foo");
        assert!(fix_endif_comment(&mut line));
        assert_eq!(line, "#endif  // foo");

        let mut line = String::from("auto pair = make_pair<int, int>(1, 2);");
        assert!(fix_make_pair(&mut line));
        assert_eq!(line, "auto pair = make_pair(1, 2);");

        let mut line = String::from("memset(buf, size, 0);");
        assert!(fix_memset(&mut line));
        assert_eq!(line, "memset(buf, 0, size);");

        let mut line = String::from("printf(\"%q\", value);");
        assert!(fix_printf_format(
            &mut line,
            &crate::messages::LintMessage::PrintfFormatDeprecatedQ
        ));
        assert_eq!(line, "printf(\"%ll\", value);");

        let mut line = String::from("const char* format = \"%q\"; printf(\"%q\", value);");
        assert!(fix_printf_format(
            &mut line,
            &crate::messages::LintMessage::PrintfFormatDeprecatedQ
        ));
        assert_eq!(line, "const char* format = \"%q\"; printf(\"%ll\", value);");

        let mut line = String::from("  namespace demo");
        assert!(fix_namespace_indentation(&mut line));
        assert_eq!(line, "namespace demo");

        let mut line = String::from("int static number;");
        assert!(fix_storage_class(&mut line));
        assert_eq!(line, "static int number;");

        let mut line = String::from("static const std::string kName = \"hello\";");
        assert!(fix_global_const_string(
            &mut line,
            "static const ",
            "",
            "kName"
        ));
        assert_eq!(line, "static const char kName[] = \"hello\";");

        let mut line = String::from("static const std::string kFactory = BuildLabel(\"hello\");");
        assert!(!fix_global_const_string(
            &mut line,
            "static const ",
            "",
            "kFactory"
        ));

        let mut line = String::from("static const std::string kCtor(\"hello\");");
        assert!(fix_global_const_string(
            &mut line,
            "static const ",
            "",
            "kCtor"
        ));
        assert_eq!(line, "static const char kCtor[] = \"hello\";");

        assert_eq!(normalize_cast_type("( char* )"), "char*");
    }

    #[test]
    fn helper_fixers_cover_parsing_and_macro_rewrites() {
        let mut line = String::from("return x;y");
        assert!(fix_semicolon_spacing(
            &mut line,
            &crate::messages::LintMessage::MissingSpaceBeforeSemicolon
        ));
        assert_eq!(line, "return x; y");

        let mut line = String::from("int x ; y");
        assert!(fix_semicolon_spacing(
            &mut line,
            &crate::messages::LintMessage::ExtraSpaceBeforeSemicolon
        ));
        assert_eq!(line, "int x; y");

        let mut line = String::from("if (ready):;");
        assert!(fix_semicolon_spacing(
            &mut line,
            &crate::messages::LintMessage::SemicolonDefiningEmptyStatementUseBraces
        ));
        assert_eq!(line, "if (ready): {}");

        let mut line = String::from("    ;");
        assert!(fix_semicolon_spacing(
            &mut line,
            &crate::messages::LintMessage::LineContainsOnlySemicolonUseBraces
        ));
        assert_eq!(line, "    {}");

        let mut line = String::from("if (ready) ;");
        assert!(fix_semicolon_spacing(
            &mut line,
            &crate::messages::LintMessage::ExtraSpaceBeforeLastSemicolonUseBraces
        ));
        assert_eq!(line, "if (ready);");

        let mut line = String::from("if (ready);");
        assert!(fix_empty_control_body(&mut line, &["if", "while"]));
        assert_eq!(line, "if (ready) {}");

        let line = String::from("if (ready) {\n  work();\n}");
        let mut lines = vec![line.clone()];
        assert!(!fix_empty_if_body(&mut lines, 0));
        assert_eq!(lines[0], line);

        let mut lines = vec![String::from("if (ready) {"), String::from("}")];
        assert!(fix_empty_if_body(&mut lines, 0));
        assert_eq!(lines, vec![String::from("if (ready) {}")]);

        let mut line = String::from("if(ready)");
        assert_eq!(normalize_control_parentheses(&line), "if (ready)");
        line = String::from("foo( bar )");
        assert_eq!(normalize_control_parentheses(&line), "foo(bar)");

        let mut line = String::from("x = 1 and y or z");
        assert!(fix_alt_tokens(&mut line));
        assert_eq!(line, "x = 1 && y || z");

        assert_eq!(remove_spaces_after_unary_operator("! value", "!"), "!value");
        assert_eq!(remove_spaces_after_unary_operator("~ value", "~"), "~value");

        let mut line = String::from("CHECK(value == \"kFoo\")");
        assert!(fix_check_macro(&mut line));
        assert_eq!(line, "CHECK_EQ(value, \"kFoo\")");

        let mut line = String::from("EXPECT_FALSE(0 != \"x\")");
        assert!(fix_check_macro(&mut line));
        assert_eq!(line, "EXPECT_EQ(0, \"x\")");

        let mut line = String::from("CHECK(x && y)");
        assert!(!fix_check_macro(&mut line));
        assert_eq!(line, "CHECK(x && y)");

        let cases = [
            (
                crate::messages::CheckMacroName::Dcheck,
                crate::messages::ComparisonOperator::Eq,
                crate::messages::CheckMacroReplacement::DcheckEq,
            ),
            (
                crate::messages::CheckMacroName::Check,
                crate::messages::ComparisonOperator::Ne,
                crate::messages::CheckMacroReplacement::CheckNe,
            ),
            (
                crate::messages::CheckMacroName::ExpectTrue,
                crate::messages::ComparisonOperator::Ge,
                crate::messages::CheckMacroReplacement::ExpectGe,
            ),
            (
                crate::messages::CheckMacroName::AssertTrue,
                crate::messages::ComparisonOperator::Gt,
                crate::messages::CheckMacroReplacement::AssertGt,
            ),
            (
                crate::messages::CheckMacroName::ExpectFalse,
                crate::messages::ComparisonOperator::Le,
                crate::messages::CheckMacroReplacement::ExpectGt,
            ),
            (
                crate::messages::CheckMacroName::AssertFalse,
                crate::messages::ComparisonOperator::Lt,
                crate::messages::CheckMacroReplacement::AssertGe,
            ),
        ];
        for (check_macro, op, expected) in cases {
            assert_eq!(replacement_check_macro(check_macro, op), Some(expected));
        }

        assert_eq!(
            split_comparison_expression("value == \"x\""),
            Some(("value ", crate::messages::ComparisonOperator::Eq, " \"x\""))
        );
        assert_eq!(split_comparison_expression("lhs && rhs"), None);
        assert!(is_check_const("\"x\""));
        assert!(is_check_const("'x'"));
        assert!(!is_check_const("x"));

        let line = "code // comment";
        assert_eq!(find_line_comment_start(line), Some(5));
        let line = "puts(\"// not comment\"); // comment";
        assert_eq!(
            find_line_comment_start(line),
            Some(line.find("// comment").unwrap())
        );
        let line = r#"puts("// not comment");"#;
        assert_eq!(find_line_comment_start(line), None);

        let lines = vec![
            String::from(""),
            String::from("alpha"),
            String::from(""),
            String::from("beta"),
        ];
        assert_eq!(previous_non_blank_line(&lines, 3), Some(1));
        assert_eq!(previous_non_blank_line(&lines, 1), None);

        assert_eq!(find_matching_paren("f(a(b))", 1), Some(6));
        assert_eq!(find_matching_paren("f(a(b)", 1), None);
        assert_eq!(
            find_matching_angle_bracket("std::vector<int>", 11),
            Some(15)
        );
        assert_eq!(find_matching_angle_bracket("std::vector<int", 11), None);

        assert_eq!(
            update_code("x=1 //c", |code| code.replace("=", " = ")),
            "x = 1 //c"
        );
        let mut line = String::from("x=1 //c");
        assert!(update_code_and_comment(&mut line, |code| code.replace("=", " = ")));
        assert_eq!(line, "x = 1 //c");
        assert_eq!(add_spaces_around_operator("x&&y", "&&"), "x && y");
        assert_eq!(remove_spaces_after_unary_operator("! value", "!"), "!value");
    }

    #[test]
    fn helper_fixers_cover_include_block_and_guard_helpers() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("src").join("demo.cc");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "// Copyright 2026\nint main() {}\n").unwrap();

        let mut options = Options::new();
        options.repository = root.clone();

        let lines = vec![
            String::from("// Copyright 2026"),
            String::from("/* header */"),
            String::from(""),
            String::from("#pragma once"),
            String::from(""),
            String::from("#include <string>"),
            String::from("#include \"src/demo.h\""),
            String::from("#include \"src/demo.h\""),
            String::from("int main() {}"),
        ];
        assert_eq!(header_guard_insertion_index(&lines), 3);
        assert_eq!(top_level_include_block(&lines), Some((5, 8)));
        assert_eq!(include_block_insertion_index(&lines), 5);

        let mut file_table = crate::diagnostics::FileTable::new();
        let file_id = file_table.intern("src/demo.cc");
        let diagnostics = vec![
            crate::diagnostics::Diagnostic {
                file_id,
                linenum: 1,
                category: crate::categories::Category::BuildIncludeWhatYouUse,
                confidence: 1,
                message: crate::messages::LintMessage::MissingSelfHeader {
                    file_from_repo: "src/demo.cc".into(),
                    header: "src/demo.h".into(),
                    includes_use_aliases: false,
                },
            },
            crate::diagnostics::Diagnostic {
                file_id,
                linenum: 1,
                category: crate::categories::Category::BuildIncludeWhatYouUse,
                confidence: 1,
                message: crate::messages::LintMessage::IwyuAddInclude(
                    crate::iwyu::IwyuHeader::String,
                    "name".into(),
                ),
            },
        ];

        let additions = missing_include_entries_from_diagnostics(&file, &options, &diagnostics);
        assert_eq!(
            additions
                .into_iter()
                .map(|entry| entry.raw_line)
                .collect::<Vec<_>>(),
            vec![
                String::from("#include \"src/demo.h\""),
                String::from("#include <string>"),
            ]
        );

        let mut include_lines = vec![
            String::from("// Copyright 2026"),
            String::from("#include <vector>"),
            String::from("#include <string>"),
            String::from("int main() {}"),
        ];
        assert_eq!(top_level_include_block(&include_lines), Some((1, 3)));
        assert!(fix_include_block(
            &file,
            &options,
            &diagnostics,
            &mut include_lines
        ));
        assert_eq!(
            include_lines,
            vec![
                String::from("// Copyright 2026"),
                String::from("#include \"src/demo.h\""),
                String::from("#include <string>"),
                String::from("#include <vector>"),
                String::from("int main() {}"),
            ]
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn helper_fixers_cover_class_layout_adjustments() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.h");
        std::fs::write(
            &file,
            concat!(
                "// Copyright 2026\n",
                "class Foo {\n",
                " public:\n",
                "  int value;\n",
                " };\n",
            ),
        )
        .unwrap();

        let options = Options::new();
        let mut lines = vec![
            "// Copyright 2026".to_string(),
            "class Foo {".to_string(),
            "   public:".to_string(),
            "  int value;".to_string(),
            " };".to_string(),
        ];

        assert!(fix_access_specifier_indentation(
            &file, &options, &mut lines, 2
        ));
        assert_eq!(lines[2], " public:");

        assert!(fix_class_closing_brace_alignment(
            &file, &options, &mut lines, 4
        ));
        assert_eq!(lines[4], "};");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_is_noop_for_dash_input() {
        let options = Options::new();
        assert!(!fix_file_in_place(Path::new("-"), &options).unwrap());
    }

    #[test]
    fn fix_file_returns_false_for_clean_file() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(&file, "// Copyright 2026\nint main() {}\n").unwrap();

        let options = Options::new();
        assert!(!fix_file_in_place(&file, &options).unwrap());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fix_file_returns_false_for_invalid_utf8_input() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        std::fs::write(&file, b"\xFF\xFE\xFA\n").unwrap();

        let options = Options::new();
        assert!(!fix_file_in_place(&file, &options).unwrap());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fix_file_reports_write_failure_for_read_only_file() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let file = root.join("sample.cc");
        fs::write(&file, "int x=0;\n").unwrap();

        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&file, permissions).unwrap();

        let options = Options::new();
        let result = fix_file_in_place(&file, &options);

        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&file, permissions).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert!(result.is_err());
    }
}
