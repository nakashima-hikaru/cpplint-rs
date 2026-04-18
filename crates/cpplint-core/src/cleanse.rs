use crate::options::Options;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use bitflags::bitflags;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use std::borrow::Cow;
use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;
use std::sync::LazyLock;

static INCLUDE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^\s*#\s*include\s*([<"])([^>"]*)[>"].*$"#).unwrap());
static ESCAPE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"\\([abfnrtv?"\\\']|\d+|x[0-9a-fA-F]+)"#).unwrap());

const ALT_TOKEN_REPLACEMENT: &[(&str, &str)] = &[
    ("and", "&&"),
    ("and_eq", "&="),
    ("bitand", "&"),
    ("bitor", "|"),
    ("compl", "~"),
    ("not", "!"),
    ("not_eq", "!="),
    ("or", "||"),
    ("or_eq", "|="),
    ("xor", "^"),
    ("xor_eq", "^="),
];

static ALT_TOKEN_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(ALT_TOKEN_REPLACEMENT.iter().map(|(token, _)| *token))
        .unwrap()
});

const KEYWORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "case",
    "default",
    "return",
    "new",
    "delete",
    "catch",
    "operator",
    "__VA_OPT__",
    "public",
    "protected",
    "private",
    "signals",
    "slots",
    "sizeof",
    "elif",
    "typedef",
    "using",
    "static_cast",
    "reinterpret_cast",
    "const_cast",
    "else",
    "do",
    "namespace",
    "virtual",
    "override",
    "final",
    "inline",
    "constexpr",
    "static",
];

static KEYWORDS_AC: LazyLock<AhoCorasick> = LazyLock::new(|| AhoCorasick::new(KEYWORDS).unwrap());

bitflags! {
    #[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
    pub struct MatchedKeywords: u32 {
        const IF          = 1 << 0;
        const FOR         = 1 << 1;
        const WHILE       = 1 << 2;
        const SWITCH      = 1 << 3;
        const CASE        = 1 << 4;
        const DEFAULT     = 1 << 5;
        const RETURN      = 1 << 6;
        const NEW         = 1 << 7;
        const DELETE      = 1 << 8;
        const CATCH       = 1 << 9;
        const OPERATOR    = 1 << 10;
        const VA_OPT      = 1 << 11;
        const ACCESS      = 1 << 12;
        const SIZEOF      = 1 << 13;
        const ELIF        = 1 << 14;
        const TYPEDEF     = 1 << 15;
        const USING       = 1 << 16;
        const CAST        = 1 << 17;
        const ELSE        = 1 << 18;
        const DO          = 1 << 19;
        const NAMESPACE   = 1 << 20;
        const VIRTUAL     = 1 << 21;
        const OVERRIDE    = 1 << 22;
        const FINAL       = 1 << 23;
        const INLINE      = 1 << 24;
        const CONSTEXPR   = 1 << 25;
        const STATIC      = 1 << 26;
        const HAS_ALT_TOKEN = 1 << 27;
    }
}

bitflags! {
    #[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
    pub struct LineFeatures: u16 {
        const COLON          = 1 << 0;
        const PAREN          = 1 << 1;
        const COMMA          = 1 << 2;
        const SEMI           = 1 << 3;
        const BRACE          = 1 << 4;
        const BRACKET        = 1 << 5;
        const OP             = 1 << 6;
        const AMP            = 1 << 7;
        const PLUS_MINUS     = 1 << 8;
        const ANGLE_QUESTION = 1 << 9;
        const HASH           = 1 << 10;
    }
}

static LINE_FEATURE_LUT: [u16; 256] = {
    let mut lut = [0; 256];
    lut[b':' as usize] |= 1 << 0;
    lut[b'(' as usize] |= 1 << 1;
    lut[b')' as usize] |= 1 << 1;
    lut[b',' as usize] |= 1 << 2;
    lut[b';' as usize] |= 1 << 3;
    lut[b'{' as usize] |= 1 << 4;
    lut[b'}' as usize] |= 1 << 4;
    lut[b'[' as usize] |= 1 << 5;
    lut[b']' as usize] |= 1 << 5;
    lut[b'=' as usize] |= 1 << 6;
    lut[b'<' as usize] |= (1 << 6) | (1 << 9);
    lut[b'>' as usize] |= (1 << 6) | (1 << 9);
    lut[b'!' as usize] |= 1 << 6;
    lut[b'~' as usize] |= 1 << 6;
    lut[b'+' as usize] |= (1 << 6) | (1 << 8);
    lut[b'-' as usize] |= (1 << 6) | (1 << 8);
    lut[b'*' as usize] |= 1 << 6;
    lut[b'/' as usize] |= 1 << 6;
    lut[b'%' as usize] |= 1 << 6;
    lut[b'&' as usize] |= (1 << 6) | (1 << 7);
    lut[b'|' as usize] |= 1 << 6;
    lut[b'^' as usize] |= 1 << 6;
    lut[b'?' as usize] |= 1 << 9;
    lut[b'#' as usize] |= 1 << 10;
    lut
};

impl MatchedKeywords {
    pub fn from_line(line: &str) -> Self {
        if !line.bytes().any(|b| b.is_ascii_alphabetic()) {
            return Self::default();
        }
        let mut bits = Self::empty();
        for mat in KEYWORDS_AC.find_iter(line) {
            bits |= match mat.pattern().as_usize() {
                0 => Self::IF,
                1 => Self::FOR,
                2 => Self::WHILE,
                3 => Self::SWITCH,
                4 => Self::CASE,
                5 => Self::DEFAULT,
                6 => Self::RETURN,
                7 => Self::NEW,
                8 => Self::DELETE,
                9 => Self::CATCH,
                10 => Self::OPERATOR,
                11 => Self::VA_OPT,
                12..=16 => Self::ACCESS,
                17 => Self::SIZEOF,
                18 => Self::ELIF,
                19 => Self::TYPEDEF,
                20 => Self::USING,
                21..=23 => Self::CAST,
                24 => Self::ELSE,
                25 => Self::DO,
                26 => Self::NAMESPACE,
                27 => Self::VIRTUAL,
                28 => Self::OVERRIDE,
                29 => Self::FINAL,
                30 => Self::INLINE,
                31 => Self::CONSTEXPR,
                32 => Self::STATIC,
                _ => Self::empty(),
            };
        }
        bits
    }

    #[inline(always)]
    pub fn has_if(&self) -> bool {
        self.contains(Self::IF)
    }
    #[inline(always)]
    pub fn has_for(&self) -> bool {
        self.contains(Self::FOR)
    }
    #[inline(always)]
    pub fn has_while(&self) -> bool {
        self.contains(Self::WHILE)
    }
    #[inline(always)]
    pub fn has_switch(&self) -> bool {
        self.contains(Self::SWITCH)
    }
    #[inline(always)]
    pub fn has_case(&self) -> bool {
        self.contains(Self::CASE)
    }
    #[inline(always)]
    pub fn has_default(&self) -> bool {
        self.contains(Self::DEFAULT)
    }
    #[inline(always)]
    pub fn has_return(&self) -> bool {
        self.contains(Self::RETURN)
    }
    #[inline(always)]
    pub fn has_new(&self) -> bool {
        self.contains(Self::NEW)
    }
    #[inline(always)]
    pub fn has_delete(&self) -> bool {
        self.contains(Self::DELETE)
    }
    #[inline(always)]
    pub fn has_catch(&self) -> bool {
        self.contains(Self::CATCH)
    }
    #[inline(always)]
    pub fn has_operator(&self) -> bool {
        self.contains(Self::OPERATOR)
    }
    #[inline(always)]
    pub fn has_va_opt(&self) -> bool {
        self.contains(Self::VA_OPT)
    }
    #[inline(always)]
    pub fn has_access(&self) -> bool {
        self.contains(Self::ACCESS)
    }
    #[inline(always)]
    pub fn has_sizeof(&self) -> bool {
        self.contains(Self::SIZEOF)
    }
    #[inline(always)]
    pub fn has_elif(&self) -> bool {
        self.contains(Self::ELIF)
    }
    #[inline(always)]
    pub fn has_typedef(&self) -> bool {
        self.contains(Self::TYPEDEF)
    }
    #[inline(always)]
    pub fn has_using(&self) -> bool {
        self.contains(Self::USING)
    }
    #[inline(always)]
    pub fn has_else(&self) -> bool {
        self.contains(Self::ELSE)
    }
    #[inline(always)]
    pub fn has_do(&self) -> bool {
        self.contains(Self::DO)
    }
    #[inline(always)]
    pub fn has_any_cast(&self) -> bool {
        self.contains(Self::CAST)
    }
    #[inline(always)]
    pub fn has_namespace(&self) -> bool {
        self.contains(Self::NAMESPACE)
    }
    #[inline(always)]
    pub fn has_virtual(&self) -> bool {
        self.contains(Self::VIRTUAL)
    }
    #[inline(always)]
    pub fn has_override(&self) -> bool {
        self.contains(Self::OVERRIDE)
    }
    #[inline(always)]
    pub fn has_final(&self) -> bool {
        self.contains(Self::FINAL)
    }
    #[inline(always)]
    pub fn has_inline(&self) -> bool {
        self.contains(Self::INLINE)
    }
    #[inline(always)]
    pub fn has_constexpr(&self) -> bool {
        self.contains(Self::CONSTEXPR)
    }
    #[inline(always)]
    pub fn has_static(&self) -> bool {
        self.contains(Self::STATIC)
    }
    #[inline(always)]
    pub fn has_alt_token(&self) -> bool {
        self.contains(Self::HAS_ALT_TOKEN)
    }

    #[inline(always)]
    pub fn has_any_control_struct(&self) -> bool {
        const MASK: MatchedKeywords = MatchedKeywords::from_bits_truncate(
            MatchedKeywords::IF.bits()
                | MatchedKeywords::ELIF.bits()
                | MatchedKeywords::FOR.bits()
                | MatchedKeywords::WHILE.bits()
                | MatchedKeywords::SWITCH.bits()
                | MatchedKeywords::RETURN.bits()
                | MatchedKeywords::NEW.bits()
                | MatchedKeywords::DELETE.bits()
                | MatchedKeywords::CATCH.bits()
                | MatchedKeywords::SIZEOF.bits(),
        );
        self.intersects(MASK)
    }
}

const RAW_STRING_PREFIXES: &[&str] = &["u8R\"", "uR\"", "UR\"", "LR\"", "R\""];
static RAW_STRING_PREFIXES_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(RAW_STRING_PREFIXES)
        .unwrap()
});

fn is_valid_alt_token_match(bytes: &[u8], start: usize, end: usize) -> bool {
    start > 0
        && matches!(bytes[start - 1], b' ' | b'=' | b'(')
        && (end == bytes.len() || matches!(bytes[end], b' ' | b'('))
}

pub fn find_alternate_tokens(line: &str) -> Vec<(&'static str, &'static str)> {
    let bytes = line.as_bytes();
    let mut matches = Vec::new();
    for mat in ALT_TOKEN_AC.find_iter(line) {
        let start = mat.start();
        let end = mat.end();
        if !is_valid_alt_token_match(bytes, start, end) {
            continue;
        }
        matches.push(ALT_TOKEN_REPLACEMENT[mat.pattern().as_usize()]);
    }
    matches
}

/// Returns true if the line contains at least one alternate token, without allocating.
fn has_alternate_tokens(line: &str) -> bool {
    let bytes = line.as_bytes();
    ALT_TOKEN_AC
        .find_iter(line)
        .any(|mat| is_valid_alt_token_match(bytes, mat.start(), mat.end()))
}

fn scan_line_features(line: &str) -> LineFeatures {
    let bytes = line.as_bytes();
    let mut mask = 0u16;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        if chunk.simd_eq(u8x32::splat(b':')).any() {
            mask |= LineFeatures::COLON.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'(')) | chunk.simd_eq(u8x32::splat(b')'))).any() {
            mask |= LineFeatures::PAREN.bits();
        }
        if chunk.simd_eq(u8x32::splat(b',')).any() {
            mask |= LineFeatures::COMMA.bits();
        }
        if chunk.simd_eq(u8x32::splat(b';')).any() {
            mask |= LineFeatures::SEMI.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'{')) | chunk.simd_eq(u8x32::splat(b'}'))).any() {
            mask |= LineFeatures::BRACE.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'[')) | chunk.simd_eq(u8x32::splat(b']'))).any() {
            mask |= LineFeatures::BRACKET.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'='))
            | chunk.simd_eq(u8x32::splat(b'<'))
            | chunk.simd_eq(u8x32::splat(b'>'))
            | chunk.simd_eq(u8x32::splat(b'!'))
            | chunk.simd_eq(u8x32::splat(b'~'))
            | chunk.simd_eq(u8x32::splat(b'+'))
            | chunk.simd_eq(u8x32::splat(b'-'))
            | chunk.simd_eq(u8x32::splat(b'*'))
            | chunk.simd_eq(u8x32::splat(b'/'))
            | chunk.simd_eq(u8x32::splat(b'%'))
            | chunk.simd_eq(u8x32::splat(b'&'))
            | chunk.simd_eq(u8x32::splat(b'|'))
            | chunk.simd_eq(u8x32::splat(b'^')))
        .any()
        {
            mask |= LineFeatures::OP.bits();
        }
        if chunk.simd_eq(u8x32::splat(b'&')).any() {
            mask |= LineFeatures::AMP.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'+')) | chunk.simd_eq(u8x32::splat(b'-'))).any() {
            mask |= LineFeatures::PLUS_MINUS.bits();
        }
        if (chunk.simd_eq(u8x32::splat(b'<'))
            | chunk.simd_eq(u8x32::splat(b'>'))
            | chunk.simd_eq(u8x32::splat(b'?')))
        .any()
        {
            mask |= LineFeatures::ANGLE_QUESTION.bits();
        }
        if chunk.simd_eq(u8x32::splat(b'#')).any() {
            mask |= LineFeatures::HASH.bits();
        }
        i += 32;
    }
    for &b in &bytes[i..] {
        mask |= LINE_FEATURE_LUT[b as usize];
    }
    LineFeatures::from_bits_retain(mask)
}

pub struct CleansedLines<'a> {
    pub raw_lines: BumpVec<'a, &'a str>,
    pub lines: BumpVec<'a, &'a str>,
    pub elided: BumpVec<'a, &'a str>,
    pub lines_without_raw_strings: BumpVec<'a, &'a str>,
    pub has_comment: BumpVec<'a, bool>,
    pub(crate) line_features: BumpVec<'a, LineFeatures>,
    pub(crate) keywords: BumpVec<'a, MatchedKeywords>,
    pub(crate) elided_without_alternate_tokens: Option<BumpVec<'a, &'a str>>,
}

impl<'a> CleansedLines<'a> {
    pub fn new(arena: &'a Bump, raw_lines: &[&'a str]) -> Self {
        Self::new_with_options(arena, raw_lines, &Options::new(), "")
    }

    pub fn new_with_options(
        arena: &'a Bump,
        raw_lines: &[&'a str],
        options: &Options,
        filename: &str,
    ) -> Self {
        let n = raw_lines.len();
        let mut lines = BumpVec::with_capacity_in(n, arena);
        let mut elided = BumpVec::with_capacity_in(n, arena);
        let mut has_comment = BumpVec::with_capacity_in(n, arena);
        let mut lines_without_raw_strings = BumpVec::with_capacity_in(n, arena);
        let mut line_features = BumpVec::with_capacity_in(n, arena);
        let mut keywords = BumpVec::with_capacity_in(n, arena);
        let mut raw_lines_arena = BumpVec::with_capacity_in(n, arena);
        raw_lines_arena.extend_from_slice(raw_lines);

        let mut in_block_comment = false;
        let mut raw_delimiter = String::new();
        let replace_alt_tokens = !options.should_print_error(
            crate::categories::Category::ReadabilityAltTokens,
            filename,
            0,
        );
        let mut elided_without_alternate_tokens =
            replace_alt_tokens.then(|| BumpVec::with_capacity_in(n, arena));

        for &raw_line_ref in raw_lines {
            // 1. Cleanse raw strings
            let mut line_without_raw: Cow<'_, str> = Cow::Borrowed(raw_line_ref);

            if !raw_delimiter.is_empty() {
                if let Some(pos) = raw_line_ref.find(&raw_delimiter) {
                    let leading_space_count = raw_line_ref
                        .bytes()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    let mut s = String::with_capacity(
                        leading_space_count + 2 + raw_line_ref.len() - (pos + raw_delimiter.len()),
                    );
                    for _ in 0..leading_space_count {
                        s.push(' ');
                    }
                    s.push_str("\"\"");
                    s.push_str(&raw_line_ref[pos + raw_delimiter.len()..]);
                    line_without_raw = Cow::Owned(s);
                    raw_delimiter.clear();
                } else {
                    line_without_raw = Cow::Borrowed("\"\"");
                }
            }

            while raw_delimiter.is_empty() {
                let Some((prefix, delimiter_text, suffix)) =
                    find_raw_string_start(&line_without_raw)
                else {
                    break;
                };

                if prefix_is_in_comment_or_literal(prefix) {
                    break;
                }

                raw_delimiter.clear();
                raw_delimiter.push(')');
                raw_delimiter.push_str(delimiter_text);
                raw_delimiter.push('"');

                if let Some(end) = suffix.find(&raw_delimiter) {
                    let mut s = String::with_capacity(
                        prefix.len() + 2 + suffix.len() - (end + raw_delimiter.len()),
                    );
                    s.push_str(prefix);
                    s.push_str("\"\"");
                    s.push_str(&suffix[end + raw_delimiter.len()..]);
                    line_without_raw = Cow::Owned(s);
                    raw_delimiter.clear();
                } else {
                    let mut s = String::with_capacity(prefix.len() + 2);
                    s.push_str(prefix);
                    s.push_str("\"\"");
                    line_without_raw = Cow::Owned(s);
                }
            }

            let was_raw_string_replaced = matches!(line_without_raw, Cow::Owned(_));
            let line_without_raw_ref: &'a str = if was_raw_string_replaced {
                arena.alloc_str(line_without_raw.as_ref())
            } else {
                raw_line_ref
            };
            lines_without_raw_strings.push(line_without_raw_ref);

            // 2. Cleanse comments
            let (comment_removed, is_comment, still_in_block) =
                cleanse_comments_line(line_without_raw_ref, in_block_comment);

            let line_comment_removed_ref: &'a str = if let Cow::Owned(s) = comment_removed {
                arena.alloc_str(&s)
            } else {
                line_without_raw_ref
            };
            lines.push(line_comment_removed_ref);
            has_comment.push(is_comment);
            in_block_comment = still_in_block;

            // 3. Collapse strings
            let collapsed_line = collapse_strings(line_comment_removed_ref);
            let line_collapsed_ref: &'a str = if let Cow::Owned(s) = collapsed_line {
                arena.alloc_str(s.as_ref())
            } else {
                line_comment_removed_ref
            };

            let has_alt = has_alternate_tokens(line_collapsed_ref);

            if let Some(lines_without_alt_tokens) = &mut elided_without_alternate_tokens {
                let elided_line = replace_alternate_tokens(line_collapsed_ref);
                lines_without_alt_tokens.push(line_collapsed_ref);

                let line_elided_ref: &'a str = if let Cow::Owned(s) = elided_line {
                    arena.alloc_str(&s)
                } else {
                    line_collapsed_ref
                };

                let mut bits = MatchedKeywords::from_line(line_elided_ref);
                if has_alt {
                    bits |= MatchedKeywords::HAS_ALT_TOKEN;
                }
                line_features.push(scan_line_features(line_elided_ref));
                keywords.push(bits);
                elided.push(line_elided_ref);
            } else {
                let elided_line = line_collapsed_ref;
                elided.push(elided_line);
                let mut bits = MatchedKeywords::from_line(elided_line);
                if has_alt {
                    bits |= MatchedKeywords::HAS_ALT_TOKEN;
                }
                line_features.push(scan_line_features(elided_line));
                keywords.push(bits);
            }
        }

        CleansedLines {
            raw_lines: raw_lines_arena,
            lines,
            elided,
            lines_without_raw_strings,
            has_comment,
            line_features,
            keywords,
            elided_without_alternate_tokens,
        }
    }

    pub fn line_without_alternate_tokens(&self, linenum: usize) -> &str {
        self.elided_without_alternate_tokens
            .as_ref()
            .and_then(|lines| lines.get(linenum))
            .copied()
            .unwrap_or(self.elided[linenum])
    }

    pub fn keywords(&self, linenum: usize) -> MatchedKeywords {
        self.keywords[linenum]
    }
}

pub fn cleanse_raw_strings(raw_lines: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(raw_lines.len());
    let mut delimiter = String::new();

    for line in raw_lines {
        let mut new_line = line.clone();

        if !delimiter.is_empty() {
            if let Some(pos) = line.find(&delimiter) {
                // End of raw string
                // Match leading space
                let leading_space = line
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .collect::<String>();
                new_line = format!("{}\"\"{}", leading_space, &line[pos + delimiter.len()..]);
                delimiter.clear();
            } else {
                new_line = "\"\"".to_string();
            }
        }

        while delimiter.is_empty() {
            let Some((prefix, raw_delimiter, suffix)) = find_raw_string_start(&new_line) else {
                break;
            };

            if prefix_is_in_comment_or_literal(prefix) {
                break;
            }

            delimiter = format!("){}\"", raw_delimiter);
            if let Some(end) = suffix.find(&delimiter) {
                new_line = format!("{}\"\"{}", prefix, &suffix[end + delimiter.len()..]);
                delimiter.clear();
            } else {
                new_line = format!("{}\"\"", prefix);
            }
        }
        result.push(new_line);
    }
    result
}

fn find_raw_string_start(line: &str) -> Option<(&str, &str, &str)> {
    for mat in RAW_STRING_PREFIXES_AC.find_iter(line) {
        let start = mat.start();
        let prefix = RAW_STRING_PREFIXES[mat.pattern()];
        if !has_raw_string_word_boundary(line, start) {
            continue;
        }

        let before = &line[..start];
        let after_prefix = &line[start + prefix.len()..];
        let Some(open_paren) = after_prefix.find('(') else {
            continue;
        };
        let raw_delimiter = &after_prefix[..open_paren];
        if raw_delimiter
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '\\' || ch == '(' || ch == ')')
        {
            continue;
        }
        let suffix = &after_prefix[open_paren + 1..];
        return Some((before, raw_delimiter, suffix));
    }
    None
}

fn has_raw_string_word_boundary(line: &str, start: usize) -> bool {
    if start == 0 {
        return true;
    }
    line[..start]
        .chars()
        .last()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
}

fn prefix_is_in_comment_or_literal(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut it = prefix.chars().peekable();
    while let Some(ch) = it.next() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '/' && it.peek() == Some(&'/') && !in_single && !in_double {
            return true;
        }
    }
    in_single || in_double
}

pub fn cleanse_comments(line: &str) -> (String, bool) {
    let (mut lines, has_comment) = cleanse_comments_from_lines(&[line.to_string()]);
    (lines.remove(0), has_comment[0])
}

fn cleanse_comments_from_lines(lines: &[String]) -> (Vec<String>, Vec<bool>) {
    let mut result = Vec::with_capacity(lines.len());
    let mut has_comment = Vec::with_capacity(lines.len());
    let mut in_block_comment = false;

    for line in lines {
        let (comment_removed, is_comment, still_in_block) =
            cleanse_comments_line(line, in_block_comment);
        result.push(comment_removed.into_owned());
        has_comment.push(is_comment);
        in_block_comment = still_in_block;
    }

    (result, has_comment)
}

fn cleanse_comments_line<'a>(
    line: &'a str,
    mut in_block_comment: bool,
) -> (Cow<'a, str>, bool, bool) {
    if line.is_empty() {
        return (Cow::Borrowed(""), false, in_block_comment);
    }

    // Quick check if we need to do anything.
    // If we're not in a block comment and the line has no interesting characters, return as-is (possibly trimmed)
    if !in_block_comment {
        let bytes = line.as_bytes();
        let mut has_special = false;
        let mut i = 0;
        while i + 32 <= bytes.len() {
            let chunk = u8x32::from_slice(&bytes[i..i + 32]);
            if (chunk.simd_eq(u8x32::splat(b'/'))
                | chunk.simd_eq(u8x32::splat(b'*'))
                | chunk.simd_eq(u8x32::splat(b'"'))
                | chunk.simd_eq(u8x32::splat(b'\''))
                | chunk.simd_eq(u8x32::splat(b'\\')))
            .any()
            {
                has_special = true;
                break;
            }
            i += 32;
        }
        if !has_special {
            for &b in &bytes[i..] {
                if matches!(b, b'/' | b'*' | b'"' | b'\'' | b'\\') {
                    has_special = true;
                    break;
                }
            }
        }
        if !has_special {
            let trimmed = line.trim_end();
            return (Cow::Borrowed(trimmed), false, false);
        }
    }

    let mut result = String::with_capacity(line.len());
    let mut is_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut just_closed_block_comment = false;

    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_block_comment {
            is_comment = true;
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                i += 2;
                in_block_comment = false;
                just_closed_block_comment = true;
                continue;
            }
            i += 1;
            continue;
        }

        if escaped {
            result.push(b as char);
            escaped = false;
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if just_closed_block_comment
            && b.is_ascii_whitespace()
            && (result.is_empty()
                || result
                    .as_bytes()
                    .last()
                    .is_some_and(|&last| last.is_ascii_whitespace()))
        {
            i += 1;
            continue;
        }

        if b == b'\\' && (in_string || in_char) {
            result.push('\\');
            escaped = true;
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if b == b'"' && !in_char {
            in_string = !in_string;
            result.push('"');
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if b == b'\'' && !in_string {
            in_char = !in_char;
            result.push('\'');
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if !in_string && !in_char && b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                is_comment = true;
                break;
            }
            if bytes[i + 1] == b'*' {
                in_block_comment = true;
                is_comment = true;
                i += 2;
                continue;
            }
        }

        result.push(b as char);
        just_closed_block_comment = false;
        i += 1;
    }

    if !is_comment
        && !in_block_comment
        && !escaped
        && !in_string
        && !in_char
        && result.len() == line.trim_end().len()
    {
        return (Cow::Borrowed(line.trim_end()), false, false);
    }

    (
        Cow::Owned(result.trim_end().to_string()),
        is_comment,
        in_block_comment,
    )
}

pub fn is_cpp_string(line: &str) -> bool {
    let mut escaped = false;
    let mut in_string = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_string = !in_string;
        }
    }
    in_string
}

pub fn collapse_strings<'a>(elided: &'a str) -> Cow<'a, str> {
    if elided.trim_start().starts_with('#') && INCLUDE_RE.is_match(elided) {
        return Cow::Borrowed(elided);
    }

    let bytes = elided.as_bytes();
    let mut has_backslash = false;
    let mut has_quote = false;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        has_backslash |= chunk.simd_eq(u8x32::splat(b'\\')).any();
        has_quote |= (chunk.simd_eq(u8x32::splat(b'"')) | chunk.simd_eq(u8x32::splat(b'\''))).any();
        if has_backslash && has_quote {
            break;
        }
        i += 32;
    }
    if !has_backslash || !has_quote {
        for &b in &bytes[i..] {
            if b == b'\\' {
                has_backslash = true;
            } else if b == b'"' || b == b'\'' {
                has_quote = true;
            }
        }
    }

    if !has_backslash && !has_quote {
        return Cow::Borrowed(elided);
    }

    // Remove escapes — only needed when both backslash and quotes are present
    let result = if has_backslash && has_quote {
        Cow::Owned(ESCAPE_RE.replace_all(elided, "").to_string())
    } else {
        Cow::Borrowed(elided)
    };

    let collapsed = collapse_quotes_and_separators(&result);
    if collapsed.len() == result.len() {
        result
    } else {
        Cow::Owned(collapsed)
    }
}

pub fn replace_alternate_tokens<'a>(line: &'a str) -> Cow<'a, str> {
    let bytes = line.as_bytes();
    let mut last = 0usize;
    let mut result = String::new();

    for mat in ALT_TOKEN_AC.find_iter(line) {
        let start = mat.start();
        let end = mat.end();
        if !is_valid_alt_token_match(bytes, start, end) {
            continue;
        }

        if result.is_empty() {
            result.reserve(line.len());
        }

        let (token, replacement) = ALT_TOKEN_REPLACEMENT[mat.pattern().as_usize()];
        result.push_str(&line[last..start]);
        result.push_str(replacement);
        last = if end < bytes.len() && matches!(token, "not" | "compl") {
            end + 1
        } else {
            end
        };
    }

    if result.is_empty() {
        return Cow::Borrowed(line);
    }

    result.push_str(&line[last..]);
    Cow::Owned(result)
}

fn collapse_quotes_and_separators(elided: &str) -> String {
    let mut result = String::with_capacity(elided.len());
    let bytes = elided.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            let mut found = false;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'"' {
                    result.push_str("\"\"");
                    i = j + 1;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                result.push('"');
                if i + 1 < bytes.len() {
                    result.push_str(&elided[i + 1..]);
                }
                return result;
            }
            continue;
        }

        if b == b'\'' {
            // Check for digit separator
            if i > 0 && i + 1 < bytes.len() {
                let prev = bytes[i - 1];
                let next = bytes[i + 1];
                if prev.is_ascii_hexdigit() && (next.is_ascii_alphanumeric() || next == b'_') {
                    i += 1;
                    continue;
                }
            }

            let mut found = false;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    result.push_str("''");
                    i = j + 1;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                result.push('\'');
                if i + 1 < bytes.len() {
                    result.push_str(&elided[i + 1..]);
                }
                return result;
            }
            continue;
        }

        result.push(b as char);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanse_raw_strings_handles_single_and_multiline_forms() {
        let lines = vec![
            r#"auto a = R"(hello)";"#.to_string(),
            r#"auto b = R"tag(line1"#.to_string(),
            r#"line2)tag";"#.to_string(),
            r#"// R"(comment raw string should stay)"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"auto a = "";"#);
        assert_eq!(actual[1], "auto b = \"\"");
        assert_eq!(actual[2], "\"\";");
        assert_eq!(actual[3], r#"// R"(comment raw string should stay)"#);
    }

    #[test]
    fn cleanse_raw_strings_requires_word_boundary_before_prefix() {
        let lines = vec![
            r#"auto x = fooR"(hello)";"#.to_string(),
            r#"auto y = (R"(hello)");"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"auto x = fooR"(hello)";"#);
        assert_eq!(actual[1], r#"auto y = ("");"#);
    }

    #[test]
    fn cleanse_raw_strings_ignores_prefixes_inside_string_and_char_literals() {
        let lines = vec![
            r#"const char* s = "R\"(not raw)\"";"#.to_string(),
            r#"char c = 'R'; auto x = R"(raw)";"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"const char* s = "R\"(not raw)\"";"#);
        assert_eq!(actual[1], r#"char c = 'R'; auto x = "";"#);
    }

    #[test]
    fn collapse_strings_keeps_digit_separators_out_of_char_collapse() {
        assert_eq!(collapse_strings("int x = 1'000'000;"), "int x = 1000000;");
        assert_eq!(collapse_strings("char c = 'x';"), "char c = '';");
        assert_eq!(collapse_strings("auto s = \"abc\";"), "auto s = \"\";");
    }

    #[test]
    fn collapse_strings_processes_quotes_in_order_and_keeps_unmatched_tail() {
        assert_eq!(collapse_strings("'x' \"abc\""), "'' \"\"");
        assert_eq!(collapse_strings("\"unterminated"), "\"unterminated");
        assert_eq!(collapse_strings("'x' \"unterminated"), "'' \"unterminated");
    }

    #[test]
    fn replace_alternate_tokens_matches_cpp_rules() {
        assert_eq!(
            replace_alternate_tokens("if (true or false)"),
            "if (true || false)"
        );
        assert_eq!(replace_alternate_tokens("if (not ready)"), "if (!ready)");
        assert_eq!(replace_alternate_tokens("x = compl y;"), "x = ~y;");
        assert_eq!(
            replace_alternate_tokens("if (true and(foo))"),
            "if (true &&(foo))"
        );
        assert_eq!(replace_alternate_tokens("android"), "android");
    }

    #[test]
    fn cleansed_lines_normalizes_alternate_tokens_but_preserves_detection_view() {
        let arena = Bump::new();
        let mut options = Options::new();
        options.add_filter("-readability/alt_tokens");
        let lines = [
            "// Copyright 2026",
            "if (true or false) return;",
            "if (not ready) return;",
        ];

        let cleansed = CleansedLines::new_with_options(&arena, &lines, &options, "test.cpp");

        assert_eq!(
            cleansed.line_without_alternate_tokens(1),
            "if (true or false) return;"
        );
        assert_eq!(cleansed.elided[1], "if (true || false) return;");
        assert_eq!(
            cleansed.line_without_alternate_tokens(2),
            "if (not ready) return;"
        );
        assert_eq!(cleansed.elided[2], "if (!ready) return;");
    }

    #[test]
    fn cleansed_lines_preserves_alternate_tokens_when_check_is_enabled() {
        let arena = Bump::new();
        let lines = ["// Copyright 2026", "if (true or false) return;"];

        let cleansed = CleansedLines::new(&arena, &lines);

        assert_eq!(
            cleansed.line_without_alternate_tokens(1),
            "if (true or false) return;"
        );
        assert_eq!(cleansed.elided[1], "if (true or false) return;");
    }

    #[test]
    fn cleanse_comments_handles_multiline_block_comments() {
        let lines = vec![
            "int a = 0; /* start".to_string(),
            "still comment".to_string(),
            "end */ int b = 1;".to_string(),
            "const char* s = \"// not a comment\";".to_string(),
        ];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], "int a = 0;");
        assert_eq!(actual[1], "");
        assert_eq!(actual[2], "int b = 1;");
        assert_eq!(actual[3], "const char* s = \"// not a comment\";");
        assert_eq!(has_comment, vec![true, true, true, false]);
    }

    #[test]
    fn cleanse_comments_preserves_comment_markers_inside_strings_and_chars() {
        let lines = vec![
            r#"const char* slash = "/* not a comment */";"#.to_string(),
            r#"char c = '/';  // real comment"#.to_string(),
            r#"const char* line = "// not a comment";"#.to_string(),
        ];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], r#"const char* slash = "/* not a comment */";"#);
        assert_eq!(actual[1], r#"char c = '/';"#);
        assert_eq!(actual[2], r#"const char* line = "// not a comment";"#);
        assert_eq!(has_comment, vec![false, true, false]);
    }

    #[test]
    fn cleanse_comments_handles_multiple_block_comments_on_one_line() {
        let lines = vec![r#"int value = /* one */ 1 + /* two */ 2;"#.to_string()];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], "int value = 1 + 2;");
        assert_eq!(has_comment, vec![true]);
    }

    #[test]
    fn find_alternate_tokens_reports_multiple_matches() {
        let actual = find_alternate_tokens("if (true or true and (not true)) return;");
        assert_eq!(actual.len(), 3);
        assert_eq!(actual[0], ("or", "||"));
        assert_eq!(actual[1], ("and", "&&"));
        assert_eq!(actual[2], ("not", "!"));
    }
}
