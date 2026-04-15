use crate::categories::Category;
use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use crate::string_utils;
use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet};
use std::borrow::Cow;
use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;
use std::sync::LazyLock;

const BASIC_CAST_NEEDLES: [&str; 12] = [
    "(int)",
    "(float)",
    "(double)",
    "(bool)",
    "(char)",
    "(size_t)",
    "(int16_t)",
    "(uint16_t)",
    "(int32_t)",
    "(uint32_t)",
    "(int64_t)",
    "(uint64_t)",
];
static DEPRECATED_CAST_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"((?:\bnew\s+(?:const\s+)?|\S<\s*(?:const\s+)?)?\b)(int|float|double|bool|char|int16_t|uint16_t|int32_t|uint32_t|int64_t|uint64_t)(\([^)].*)"#,
    )
    .unwrap()
});
static CHAR_PTR_CAST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\((char\s?\*+\s?)\)\s*\""#).unwrap());
static ADDRESS_OF_CPP_CAST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"&\s*(?:static_cast|reinterpret_cast)\s*<[^>]+>\s*\("#).unwrap());
static ADDRESS_OF_C_CAST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"&\s*\(\s*[^()]+\*+\s*\)\s*[\w:(]"#).unwrap());
static EXPECTING_MOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*MOCK_(CONST_)?METHOD\d+(_T)?\("#).unwrap());
static EXPECTING_MOCK2_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*MOCK_(?:CONST_)?METHOD\d+(?:_T)?\((?:\S+,)?\s*$"#).unwrap());
static EXPECTING_MOCK3_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*MOCK_(?:CONST_)?METHOD\d+(?:_T)?\(\s*$"#).unwrap());
static EXPECTING_STD_FUNCTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bstd::m?function\s*<\s*$"#).unwrap());
static ARRAY_CAST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\([^()]+\)\s*\["#).unwrap());
static FUNCTION_POINTER_CAST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\((?:[^() ]+::\s*\*\s*)?[^() ]+\)\s*\("#).unwrap());
static CAST_KEYWORD_CONTEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#".*\b(?:sizeof|alignof|alignas|[_A-Z][_A-Z0-9]*)\s*$"#).unwrap());
static CAST_MACRO_CONTEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#".*\b[_A-Z][_A-Z0-9]*\s*\((?:\([^()]*\)|[^()])*$"#).unwrap());
static CAST_TRAILING_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:;|const\b|throw\b|final\b|override\b|[=>{),]|->)"#).unwrap()
});
static INITIALIZER_LIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bstd\s*::\s*initializer_list\b"#).unwrap());
static REF_TEMPLATE_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\s+<"#).unwrap());
static MEMSET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"memset\s*\(([^,]*),\s*([^,]*),\s*0\s*\)"#).unwrap());
static MEMSET_NUMERIC_SIZE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^''|-?[0-9]+|0x[0-9A-Fa-f]$"#).unwrap());
static THREADSAFE_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:(?:[-+*/=%^&|(<]\s*)|(?:>\s+))(asctime|ctime|getgrgid|getgrnam|getlogin|getpwnam|getpwuid|gmtime|localtime|rand|strtok|ttyname)\([^)]*\)"#,
    )
    .unwrap()
});
static GLOBAL_STRING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*((?:static\s+)?(?:const\s+)?)((?:::\s*)?(?:std::)?string)(\s+const)?\s+([a-zA-Z0-9_:]+)\b(.*)"#,
    )
    .unwrap()
});
static SNPRINTF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"snprintf\s*\(([^,]*),\s*([0-9]*)\s*,"#).unwrap());
static SPRINTF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\bsprintf\s*\("#).unwrap());
static ARG_LIST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\(([^()]*)\)"#).unwrap());
static VLA_DECL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*(?:.+::)?(\w+)\s+[a-z]\w*\[(.+)\];"#).unwrap());
static TOKEN_SPLIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+|\+|\-|\*|/|<<|>>|\]"#).unwrap());

const STRCPY_CAT_NEEDLES: [&str; 2] = ["strcpy(", "strcat("];
static STRCPY_CAT_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(STRCPY_CAT_NEEDLES).unwrap());

static POINTER_INCREMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*\*\w+(\+\+|--);"#).unwrap());
static MIN_MAX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(\w+|[+-]?\d+(\.\d*)?)\s*(<|>)\?=?\s*(\w+|[+-]?\d+)(\.\d*)?"#).unwrap()
});
const DECL_TYPE_NEEDLES: [&str; 20] = [
    "const", "volatile", "void", "char", "short", "int", "long", "float", "double", "signed",
    "unsigned", "schar", "int8_t", "uint8_t", "int16_t", "uint16_t", "int32_t", "uint32_t",
    "int64_t", "uint64_t",
];
const STORAGE_CLASS_NEEDLES: [&str; 4] = ["register", "static", "extern", "typedef"];

static DECL_ORDER_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let mut patterns = Vec::new();
    patterns.extend_from_slice(&DECL_TYPE_NEEDLES);
    patterns.extend_from_slice(&STORAGE_CLASS_NEEDLES);
    AhoCorasick::new(patterns).unwrap()
});

const VLOG_NEEDLES: [&str; 5] = [
    "VLOG(INFO)",
    "VLOG(ERROR)",
    "VLOG(WARNING)",
    "VLOG(DFATAL)",
    "VLOG(FATAL)",
];
static VLOG_AC: LazyLock<AhoCorasick> = LazyLock::new(|| AhoCorasick::new(VLOG_NEEDLES).unwrap());

static FORWARD_DECL_INNER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*class\s+(\w+\s*::\s*)+\w+\s*;"#).unwrap());
static ENDIF_TEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#\s*endif\s*[^/\s]+"#).unwrap());
static CONST_STRING_MEMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*const\s*string\s*&\s*\w+\s*;"#).unwrap());

static STRING_PTR_OR_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bstring\b(\s+const)?\s*[\*\&]\s*(const\s+)?\w"#).unwrap());
static GLOBAL_STRING_CTOR_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*(<.*>)?(::[a-zA-Z0-9_]+)*\s*\(([^"]|$)"#).unwrap());
static PRINTF_FORMAT_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"printf\s*\(.*".*%[-+ ]?\d*q"#, // 0: Q
        r#"printf\s*\(.*".*%\d+\$"#,      // 1: POSITIONAL
    ])
    .unwrap()
});
static UNARY_OPERATOR_AMPERSAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boperator\s*&\s*\(\s*\)"#).unwrap());
static NON_CONST_REF_CHECK_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"\b(?:swap|operator<<|operator>>)\s*\("#, // 0: EXEMPT
        r#"^\s*(?:[\w:<>]+\s*(?:::\s*[\w:<>]+)*\s*[&*]?\s+)+(?:operator[^\s(]+|[A-Za-z_~]\w*)\s*\("#, // 1: FUNCTION_DECL
    ])
    .unwrap()
});
static SIZEOF_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"sizeof\(.+\)"#).unwrap());
static ARRAYSIZE_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"arraysize\(\w+\)"#).unwrap());
static CONSTANT_MATCH_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"^0[xX][0-9A-Fa-f]+$"#,        // 0: HEX_LITERAL
        r#"^k[A-Z0-9]\w*$"#,             // 1: K_CONSTANT
        r#"^(?:.+::)?k[A-Z0-9]\w*$"#,    // 2: QUALIFIED_K_CONSTANT
        r#"^(?:.+::)?[A-Z][A-Z0-9_]*$"#, // 3: QUALIFIED_UPPER_CONSTANT
    ])
    .unwrap()
});
const THREADSAFE_FN_NEEDLES: [&str; 12] = [
    "asctime(",
    "ctime(",
    "getgrgid(",
    "getgrnam(",
    "getlogin(",
    "getpwnam(",
    "getpwuid(",
    "gmtime(",
    "localtime(",
    "rand(",
    "strtok(",
    "ttyname(",
];
static THREADSAFE_FN_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(THREADSAFE_FN_NEEDLES).unwrap());

static C_INTEGER_TYPES_NEEDLES: [&str; 3] = ["port", "short", "long"];
static C_INTEGER_TYPES_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(C_INTEGER_TYPES_NEEDLES).unwrap());
const RUNTIME_CHECK_NEEDLES: &[&str] = &[
    "explicit",
    "register",
    "static",
    "extern",
    "typedef",
    "class",
    "struct",
    "#endif",
    "memset",
    "VLOG",
    "make_pair",
    "strcpy",
    "strcat",
    "snprintf",
    "sprintf",
    "printf",
    "port",
    "short",
    "long",
    "string",
    "asctime",
    "ctime",
    "getgrgid",
    "getgrnam",
    "getlogin",
    "getpwnam",
    "getpwuid",
    "gmtime",
    "localtime",
    "rand",
    "strtok",
    "ttyname",
];

static RUNTIME_CHECK_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(RUNTIME_CHECK_NEEDLES).unwrap());

const RT_PAREN_BIT: u16 = 1 << 0;
const RT_AMP_BIT: u16 = 1 << 1;
const RT_PLUS_MINUS_BIT: u16 = 1 << 2;
const RT_ANGLE_QUESTION_BIT: u16 = 1 << 3;
const RT_HASH_BIT: u16 = 1 << 4;

static RUNTIME_LUT: [u16; 256] = {
    let mut lut = [0; 256];
    lut[b'(' as usize] |= RT_PAREN_BIT;
    lut[b')' as usize] |= RT_PAREN_BIT;
    lut[b'&' as usize] |= RT_AMP_BIT;
    lut[b'+' as usize] |= RT_PLUS_MINUS_BIT;
    lut[b'-' as usize] |= RT_PLUS_MINUS_BIT;
    lut[b'<' as usize] |= RT_ANGLE_QUESTION_BIT;
    lut[b'>' as usize] |= RT_ANGLE_QUESTION_BIT;
    lut[b'?' as usize] |= RT_ANGLE_QUESTION_BIT;
    lut[b'#' as usize] |= RT_HASH_BIT;
    lut
};

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn check(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>, linenum: usize) {
    let line = &clean_lines.lines[linenum];
    let elided_line = &clean_lines.elided[linenum];

    let bytes = elided_line.as_bytes();
    let mut mask = 0u16;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        if (chunk.simd_eq(u8x32::splat(b'(')) | chunk.simd_eq(u8x32::splat(b')'))).any() {
            mask |= RT_PAREN_BIT;
        }
        if chunk.simd_eq(u8x32::splat(b'&')).any() {
            mask |= RT_AMP_BIT;
        }
        if (chunk.simd_eq(u8x32::splat(b'+')) | chunk.simd_eq(u8x32::splat(b'-'))).any() {
            mask |= RT_PLUS_MINUS_BIT;
        }
        if (chunk.simd_eq(u8x32::splat(b'<'))
            | chunk.simd_eq(u8x32::splat(b'>'))
            | chunk.simd_eq(u8x32::splat(b'?')))
        .any()
        {
            mask |= RT_ANGLE_QUESTION_BIT;
        }
        if chunk.simd_eq(u8x32::splat(b'#')).any() {
            mask |= RT_HASH_BIT;
        }
        i += 32;
    }
    for &b in &bytes[i..] {
        mask |= RUNTIME_LUT[b as usize];
    }

    let has_paren = (mask & RT_PAREN_BIT) != 0;
    let has_ampersand = (mask & RT_AMP_BIT) != 0;
    let has_plus_minus = (mask & RT_PLUS_MINUS_BIT) != 0;
    let has_angle_question = (mask & RT_ANGLE_QUESTION_BIT) != 0;
    let has_hash = (mask & RT_HASH_BIT) != 0;

    // Keyword based skip
    let has_keyword = RUNTIME_CHECK_AC.is_match(elided_line);

    if has_paren || has_ampersand || has_keyword {
        check_casts(linter, clean_lines, elided_line, linenum);
    }
    if has_paren || has_keyword {
        check_explicit_constructors(linter, clean_lines, elided_line, linenum);
    }
    if has_plus_minus {
        check_invalid_increment(linter, elided_line, linenum);
    }
    if has_angle_question {
        check_deprecated_min_max_operators(linter, elided_line, linenum);
    }
    if has_keyword {
        check_storage_class_specifier(linter, elided_line, linenum);
        check_forward_decl(linter, elided_line, linenum);
    }
    if has_hash && has_keyword {
        check_endif_comment(linter, elided_line, linenum);
    }

    check_const_string_member(linter, elided_line, linenum);

    if has_keyword {
        check_memset(linter, elided_line, linenum);
        check_threadsafe_functions(linter, elided_line, linenum);
        check_vlog_arguments(linter, elided_line, linenum);
        check_make_pair_uses_deduction(linter, elided_line, linenum);
        check_global_strings(linter, elided_line, linenum);
    } else if elided_line.contains("string") {
        check_global_strings(linter, elided_line, linenum);
    }

    check_init_with_self(linter, clean_lines, linenum);

    if has_keyword {
        check_printf(linter, elided_line, linenum);
    }
    check_printf_format(linter, line, linenum);

    if has_ampersand {
        check_unary_operator_ampersand(linter, elided_line, linenum);
    }

    if has_keyword {
        check_c_integer_types(linter, elided_line, clean_lines.raw_lines[linenum], linenum);
    }

    if has_ampersand {
        check_non_const_references(linter, elided_line, linenum);
    }
    check_variable_length_arrays(linter, elided_line, linenum);
}

fn check_casts(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if !elided_line.contains('(') && !elided_line.contains('&') {
        return;
    }

    let address_of_cast = contains_single_ampersand_cast(&ADDRESS_OF_CPP_CAST_RE, elided_line)
        || contains_single_ampersand_cast(&ADDRESS_OF_C_CAST_RE, elided_line);
    let expecting_function = expecting_function_args(clean_lines, elided_line, linenum);
    if !address_of_cast {
        if let Some(captures) = DEPRECATED_CAST_STYLE_RE.captures(elided_line) {
            let matched_funcptr = captures.get(3).map(|m| m.as_str()).unwrap_or("");
            if !expecting_function {
                if ARRAY_CAST_RE.is_match(matched_funcptr) {
                    return;
                }

                let matched_new_or_template = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                let matched_type = captures.get(2).map(|m| m.as_str()).unwrap_or("");

                if matched_new_or_template.is_empty()
                    && !FUNCTION_POINTER_CAST_RE.is_match(matched_funcptr)
                    && !matched_funcptr.starts_with("(*)")
                    && !is_using_alias_for_type(elided_line, matched_type)
                    && !is_placement_new_of_type(elided_line, matched_type)
                {
                    linter.error(
                        linenum,
                        Category::ReadabilityCasting,
                        4,
                        &format!(
                            "Using deprecated casting style.  Use static_cast<{}>(...) instead",
                            matched_type
                        ),
                    );
                }
            }
        }

        if !expecting_function {
            let mut current_pos = 0;
            let bytes = elided_line.as_bytes();
            'outer: while let Some(idx) = memchr::memchr(b'(', &bytes[current_pos..]) {
                let start = current_pos + idx;
                if start + 1 < bytes.len() {
                    let next_byte = bytes[start + 1];
                    if matches!(
                        next_byte,
                        b'i' | b'f' | b'd' | b'b' | b'c' | b's' | b'u'
                    ) {
                        let rest = &elided_line[start..];
                        for needle in BASIC_CAST_NEEDLES {
                            if rest.starts_with(needle) {
                                let type_str = &needle[1..needle.len() - 1];
                                if check_c_style_cast_internal(
                                    linter,
                                    clean_lines,
                                    elided_line,
                                    linenum,
                                    "static_cast",
                                    type_str,
                                    start + 1,
                                    start + needle.len(),
                                ) {
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                current_pos = start + 1;
            }
        }

        if !check_c_style_cast(
            linter,
            clean_lines,
            elided_line,
            linenum,
            "const_cast",
            &CHAR_PTR_CAST_RE,
        ) {
            static REINTERPRET_CAST_RE: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r#"\((\w+\s?\*+\s?)\)"#).unwrap());
            if elided_line.contains('*') {
                check_c_style_cast(
                    linter,
                    clean_lines,
                    elided_line,
                    linenum,
                    "reinterpret_cast",
                    &REINTERPRET_CAST_RE,
                );
            }
        }
    }

    if address_of_cast {
        linter.error(
            linenum,
            Category::RuntimeCasting,
            4,
            "Are you taking an address of a cast?  This is dangerous: could be a temp var.  Take the address before doing the cast, rather than after",
        );
    }
}

fn contains_single_ampersand_cast(re: &Regex, line: &str) -> bool {
    re.find_iter(line).any(|m| {
        let prefix = &line[..m.start()];
        !prefix.ends_with('&')
    })
}

fn check_explicit_constructors(
    linter: &mut FileLinter,
    _clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    let Some(class_name) = linter.facts().nearest_class_name(linenum) else {
        return;
    };
    if !elided_line.contains(class_name) {
        return;
    }

    let Some((is_marked_explicit, args_str)) = parse_constructor_signature(elided_line, class_name)
    else {
        return;
    };
    let constructor_args = split_args(args_str);

    let defaulted_args_count = constructor_args
        .iter()
        .filter(|arg| arg.contains('='))
        .count();
    let variadic_args_count = constructor_args
        .iter()
        .filter(|arg| arg.contains("&&..."))
        .count();
    let onearg_constructor = constructor_args.len() == 1
        || (!constructor_args.is_empty() && defaulted_args_count >= constructor_args.len() - 1)
        || (constructor_args.len() <= 2 && variadic_args_count >= 1);

    let noarg_constructor = constructor_args.is_empty()
        || (constructor_args.len() == 1 && constructor_args[0].trim() == "void");
    if !onearg_constructor || noarg_constructor {
        return;
    }

    let first_arg = constructor_args.first().copied().unwrap_or("");
    if INITIALIZER_LIST_RE.is_match(first_arg) {
        return;
    }
    if is_copy_constructor_arg(first_arg, class_name)
        || is_move_constructor_arg(first_arg, class_name)
    {
        return;
    }

    if !is_marked_explicit {
        let message = if defaulted_args_count > 0 || variadic_args_count > 0 {
            "Constructors callable with one argument should be marked explicit."
        } else {
            "Single-parameter constructors should be marked explicit."
        };
        linter.error(linenum, Category::RuntimeExplicit, 4, message);
    }
}

fn expecting_function_args(
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) -> bool {
    EXPECTING_MOCK_RE.is_match(elided_line)
        || (linenum >= 2
            && (EXPECTING_MOCK2_RE.is_match(clean_lines.elided[linenum - 1])
                || EXPECTING_MOCK3_RE.is_match(clean_lines.elided[linenum - 2])
                || EXPECTING_STD_FUNCTION_RE.is_match(clean_lines.elided[linenum - 1])))
}

fn check_c_style_cast(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
    cast_type: &str,
    pattern: &Regex,
) -> bool {
    let Some(captures) = pattern.captures(elided_line) else {
        return false;
    };

    let Some(type_match) = captures.get(1) else {
        return false;
    };

    let endpos = captures.get(0).map(|m| m.end()).unwrap_or(0);
    check_c_style_cast_internal(
        linter,
        clean_lines,
        elided_line,
        linenum,
        cast_type,
        type_match.as_str(),
        type_match.start(),
        endpos,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_c_style_cast_internal(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
    cast_type: &str,
    type_str: &str,
    type_start: usize,
    match_end: usize,
) -> bool {
    let context_end = type_start.saturating_sub(1);
    let local_context = &elided_line[..context_end];
    if CAST_KEYWORD_CONTEXT_RE.is_match(local_context) {
        return false;
    }

    let mut context = Cow::Borrowed(local_context);
    if linenum > 0 {
        let min_line = linenum.saturating_sub(5);
        let mut context_len = context_end;
        for idx in min_line + 1..linenum {
            context_len += clean_lines.elided[idx].len();
        }
        let mut joined_context = String::with_capacity(context_len);
        for idx in min_line + 1..linenum {
            joined_context.push_str(clean_lines.elided[idx]);
        }
        joined_context.push_str(local_context);
        if CAST_MACRO_CONTEXT_RE.is_match(&joined_context) {
            return false;
        }
        context = Cow::Owned(joined_context);
    }

    if context.ends_with(" operator++")
        || context.ends_with(" operator--")
        || context.ends_with("::operator++")
        || context.ends_with("::operator--")
    {
        return false;
    }

    if CAST_TRAILING_TOKEN_RE.is_match(&elided_line[match_end..]) {
        return false;
    }

    linter.error(
        linenum,
        Category::ReadabilityCasting,
        4,
        &format!(
            "Using C-style cast.  Use {}<{}>(...) instead",
            cast_type, type_str
        ),
    );
    true
}

fn parse_constructor_signature<'a>(line: &'a str, class_name: &str) -> Option<(bool, &'a str)> {
    let mut remainder = line.trim();
    let mut is_explicit = false;

    loop {
        let previous = remainder;
        if let Some(rest) = remainder.strip_prefix("inline ") {
            remainder = rest.trim_start();
            continue;
        }
        if let Some(rest) = remainder.strip_prefix("constexpr ") {
            remainder = rest.trim_start();
            continue;
        }
        if !is_explicit && let Some(rest) = remainder.strip_prefix("explicit ") {
            is_explicit = true;
            remainder = rest.trim_start();
            continue;
        }
        if remainder == previous {
            break;
        }
    }

    let remainder = remainder.strip_prefix(class_name)?;
    let remainder = remainder.trim_start();
    if !remainder.starts_with('(') {
        return None;
    }

    let close = find_matching_paren(remainder)?;
    Some((is_explicit, remainder[1..close].trim()))
}

fn find_matching_paren(line: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in line.char_indices() {
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

fn split_args(args: &str) -> Vec<&str> {
    if args.trim().is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    for (idx, ch) in args.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ',' if angle_depth == 0 && paren_depth == 0 => {
                let arg = args[start..idx].trim();
                if !arg.is_empty() {
                    parts.push(arg);
                }
                start = idx + ch.len_utf8();
                continue;
            }
            _ => {}
        }
    }
    let trailing = args[start..].trim();
    if !trailing.is_empty() {
        parts.push(trailing);
    }
    parts
}

fn normalize_template_spacing(arg: &str) -> Cow<'_, str> {
    REF_TEMPLATE_SPACE_RE.replace_all(arg.trim(), "<")
}

fn strip_keyword_prefix<'a>(s: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(keyword)?;
    rest.chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace())
        .then_some(rest)
}

fn is_const_reference(s: &str) -> bool {
    let s = s.trim();
    if s.ends_with("const&") || s.ends_with("const &") {
        return true;
    }
    if !s.starts_with("const") {
        return false;
    }
    // Starts with const. Check if there are any * at top level.
    let mut depth = 0usize;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            '*' if depth == 0 => return false,
            '&' if depth == 0 && !s.ends_with(ch) => return false, // Handles const T& & (invalid but good were cautious)
            _ => {}
        }
    }
    true
}

fn strip_cv_prefix(mut s: &str) -> &str {
    loop {
        s = s.trim_start();
        if let Some(rest) = strip_keyword_prefix(s, "const") {
            s = rest;
            continue;
        }
        if let Some(rest) = strip_keyword_prefix(s, "volatile") {
            s = rest;
            continue;
        }
        return s;
    }
}

fn strip_trailing_param_name(arg: &str) -> &str {
    let trimmed = arg.trim_end();
    let bytes = trimmed.as_bytes();
    let mut end = bytes.len();
    while end > 0 && (bytes[end - 1].is_ascii_alphanumeric() || bytes[end - 1] == b'_') {
        end -= 1;
    }
    if end == bytes.len() {
        return trimmed;
    }
    if end == 0 || !bytes[end - 1].is_ascii_whitespace() {
        return trimmed;
    }
    trimmed[..end].trim_end()
}

fn strip_optional_template_args(s: &str) -> Option<&str> {
    let s = s.trim_start();
    let Some(rest) = s.strip_prefix('<') else {
        return Some(s);
    };

    let mut depth = 1usize;
    for (idx, ch) in rest.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&rest[idx + 1..]);
                }
            }
            _ => {}
        }
    }
    None
}

fn matches_constructor_arg_kind(arg: &str, class_name: &str, ref_token: &str) -> bool {
    let normalized = normalize_template_spacing(arg);
    let arg = strip_trailing_param_name(normalized.as_ref());
    let arg = strip_cv_prefix(arg);
    let Some(rest) = arg.strip_prefix(class_name) else {
        return false;
    };
    let Some(rest) = strip_optional_template_args(rest) else {
        return false;
    };
    let rest = strip_cv_prefix(rest);
    let Some(rest) = rest.strip_prefix(ref_token) else {
        return false;
    };
    rest.trim().is_empty()
}

fn is_copy_constructor_arg(arg: &str, class_name: &str) -> bool {
    matches_constructor_arg_kind(arg, class_name, "&")
}

fn is_move_constructor_arg(arg: &str, class_name: &str) -> bool {
    matches_constructor_arg_kind(arg, class_name, "&&")
}

fn is_using_alias_for_type(line: &str, matched_type: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("using ") else {
        return false;
    };
    let Some((_, rhs)) = rest.split_once('=') else {
        return false;
    };
    rhs.trim_start().starts_with(matched_type)
}

fn is_placement_new_of_type(line: &str, matched_type: &str) -> bool {
    let Some(start) = line.find("new(") else {
        return false;
    };
    let remainder = &line[start + 4..];
    let Some(close) = remainder.find(')') else {
        return false;
    };
    close > 0
        && remainder[close + 1..]
            .trim_start()
            .starts_with(matched_type)
}

fn check_invalid_increment(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !elided_line.contains('*') {
        return;
    }
    if POINTER_INCREMENT_RE.is_match(elided_line) {
        linter.error(
            linenum,
            Category::RuntimeInvalidIncrement,
            5,
            "Changing pointer instead of value (or unused value of operator*).",
        );
    }
}

fn check_deprecated_min_max_operators(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !elided_line.contains('?') || (!elided_line.contains('<') && !elided_line.contains('>')) {
        return;
    }
    if MIN_MAX_RE.is_match(elided_line) {
        linter.error(
            linenum,
            Category::BuildDeprecated,
            3,
            ">? and <? (max and min) operators are non-standard and deprecated.",
        );
    }
}

fn check_storage_class_specifier(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    let matches: Vec<_> = DECL_ORDER_AC.find_iter(elided_line).collect();
    for i in 1..matches.len() {
        let prev = &matches[i - 1];
        let curr = &matches[i];

        // Type match (ID 0-19) followed by Storage Class match (ID 20-23)
        if prev.pattern().as_usize() < 20 && curr.pattern().as_usize() >= 20 {
            let gap = &elided_line[prev.end()..curr.start()];
            if !gap.is_empty() && gap.chars().all(|c| c.is_whitespace() || c == '*') {
                // Validate word boundaries for both matches (equivalent to \b in regex)
                if string_utils::is_word_match(elided_line, prev.start(), prev.end())
                    && string_utils::is_word_match(elided_line, curr.start(), curr.end())
                {
                    linter.error(
                        linenum,
                        Category::BuildStorageClass,
                        5,
                        "Storage-class specifier (static, extern, typedef, etc) should be at the beginning of the declaration.",
                    );
                    break;
                }
            }
        }
    }
}

fn check_forward_decl(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !elided_line.contains("class") {
        return;
    }
    if FORWARD_DECL_INNER_RE.is_match(elided_line) {
        linter.error(
            linenum,
            Category::BuildForwardDecl,
            5,
            "Inner-style forward declarations are invalid.  Remove this line.",
        );
    }
}

fn check_endif_comment(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !elided_line.contains("#endif") {
        return;
    }
    if ENDIF_TEXT_RE.is_match(elided_line) {
        linter.error(
            linenum,
            Category::BuildEndifComment,
            5,
            "Uncommented text after #endif is non-standard.  Use a comment.",
        );
    }
}

fn check_const_string_member(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !elided_line.contains("string") || !elided_line.contains('&') {
        return;
    }
    if CONST_STRING_MEMBER_RE.is_match(elided_line) {
        linter.error(
            linenum,
            Category::RuntimeMemberStringReferences,
            2,
            "const string& members are dangerous. It is much better to use alternatives, such as pointers or simple constants.",
        );
    }
}

fn check_memset(linter: &mut FileLinter, line: &str, linenum: usize) {
    if !line.contains("memset") {
        return;
    }
    let Some(captures) = MEMSET_RE.captures(line) else {
        return;
    };
    let target = captures.get(1).map(|m| m.as_str().trim()).unwrap_or("");
    let size = captures.get(2).map(|m| m.as_str().trim()).unwrap_or("");
    if MEMSET_NUMERIC_SIZE_RE.is_match(size) {
        return;
    }
    linter.error(
        linenum,
        Category::RuntimeMemset,
        4,
        &format!(r#"Did you mean "memset({}, 0, {})"?"#, target, size),
    );
}

fn check_threadsafe_functions(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !THREADSAFE_FN_AC.is_match(elided_line) {
        return;
    }
    let Some(captures) = THREADSAFE_FN_RE.captures(elided_line) else {
        return;
    };
    let funcname = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    linter.error(
        linenum,
        Category::RuntimeThreadsafeFn,
        2,
        &format!(
            "Consider using {}_r(...) instead of {}(...) for improved thread safety.",
            funcname, funcname
        ),
    );
}

fn check_vlog_arguments(linter: &mut FileLinter, line: &str, linenum: usize) {
    if !line.contains("VLOG") {
        return;
    }
    if VLOG_AC.is_match(line) {
        linter.error(
            linenum,
            Category::RuntimeVlog,
            5,
            "VLOG() should be used with numeric verbosity level.  Use LOG() if you want symbolic severity levels.",
        );
    }
}

fn check_make_pair_uses_deduction(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    const NEEDLE: &str = "make_pair";
    let mut current_pos = 0;

    while let Some(start_offset) = elided_line[current_pos..].find(NEEDLE) {
        let match_start = current_pos + start_offset;
        let match_end = match_start + NEEDLE.len();

        let is_word_boundary_start = match match_start
            .checked_sub(1)
            .and_then(|i| elided_line.as_bytes().get(i))
        {
            Some(&b) => !b.is_ascii_alphanumeric() && b != b'_',
            None => true,
        };
        let is_word_boundary_end = match elided_line.as_bytes().get(match_end) {
            Some(&b) => !b.is_ascii_alphanumeric() && b != b'_',
            None => true,
        };

        if is_word_boundary_start && is_word_boundary_end {
            let mut bracket_start = match_end;
            while elided_line
                .as_bytes()
                .get(bracket_start)
                .is_some_and(|&b| b.is_ascii_whitespace())
            {
                bracket_start += 1;
            }

            if elided_line.as_bytes().get(bracket_start) == Some(&b'<') {
                linter.error(
                    linenum,
                    Category::BuildExplicitMakePair,
                    4,
                    "For C++11-compatibility, omit template arguments from make_pair OR use pair directly OR if appropriate, construct a pair directly",
                );
                return;
            }
        }
        current_pos = match_end;
    }
}

fn check_global_strings(linter: &mut FileLinter, line: &str, linenum: usize) {
    if line.starts_with(' ') || line.starts_with('\t') || !line.contains("string") {
        return;
    }

    let Some(captures) = GLOBAL_STRING_RE.captures(line) else {
        return;
    };

    if STRING_PTR_OR_REF_RE.is_match(line) || string_utils::contains_word(line, "operator") {
        return;
    }

    let tail = captures.get(5).map(|m| m.as_str()).unwrap_or("");
    if GLOBAL_STRING_CTOR_TAIL_RE.is_match(tail) {
        return;
    }

    let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let suffix_const = captures.get(3).map(|m| m.as_str()).unwrap_or("");
    let name = captures.get(4).map(|m| m.as_str()).unwrap_or("");

    if line.contains("const") {
        linter.error(
            linenum,
            Category::RuntimeString,
            4,
            &format!(
                "For a static/global string constant, use a C style string instead: \"{}char{} {}[]\".",
                prefix, suffix_const, name
            ),
        );
    } else {
        linter.error(
            linenum,
            Category::RuntimeString,
            4,
            "Static/global string variables are not permitted.",
        );
    }
}

fn check_init_with_self(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>, linenum: usize) {
    let line = &clean_lines.elided[linenum];
    if has_self_initializer(line) {
        linter.error(
            linenum,
            Category::RuntimeInit,
            4,
            "You seem to be initializing a member variable with itself.",
        );
        return;
    }
    if line.trim_start().starts_with(':') {
        return;
    }

    if !line.trim_end().ends_with(')') {
        return;
    }

    let mut initializer = String::new();
    let mut saw_colon = false;
    for next in linenum + 1..usize::min(linenum + 4, clean_lines.elided.len()) {
        let piece = clean_lines.elided[next].trim();
        if piece.is_empty() {
            continue;
        }
        if !saw_colon {
            if !piece.starts_with(':') {
                return;
            }
            saw_colon = true;
        }
        initializer.push_str(piece);
        if has_self_initializer(&initializer) {
            linter.error(
                linenum,
                Category::RuntimeInit,
                4,
                "You seem to be initializing a member variable with itself.",
            );
            return;
        }
        if piece.contains('{') || piece.ends_with(';') {
            break;
        }
    }
}

fn has_self_initializer(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if !bytes[idx].is_ascii_alphanumeric() && bytes[idx] != b'_' {
            idx += 1;
            continue;
        }

        let start = idx;
        idx += 1;
        while idx < bytes.len() && (bytes[idx].is_ascii_alphanumeric() || bytes[idx] == b'_') {
            idx += 1;
        }
        let name = &line[start..idx];
        if !name.ends_with('_') || bytes.get(idx) != Some(&b'(') {
            continue;
        }

        let mut depth = 0usize;
        let mut end = idx;
        while end < bytes.len() {
            match bytes[end] {
                b'(' => depth += 1,
                b')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let arg = line[idx + 1..end].trim();
                        let check_notnull_arg = arg
                            .strip_prefix("CHECK_NOTNULL(")
                            .and_then(|value| value.strip_suffix(')'));
                        if arg == name || check_notnull_arg == Some(name) {
                            return true;
                        }
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
    }
    false
}

fn check_printf(linter: &mut FileLinter, line: &str, linenum: usize) {
    if !line.contains("printf") && !STRCPY_CAT_AC.is_match(line) {
        return;
    }

    if let Some(mat) = STRCPY_CAT_AC.find(line) {
        let func = ["strcpy", "strcat"][mat.pattern()];
        let start = mat.start();

        let before_ok = start == 0 || !string_utils::is_word_char(line.as_bytes()[start - 1]);
        if before_ok {
            linter.error(
                linenum,
                Category::RuntimePrintf,
                4,
                &format!("Almost always, snprintf is better than {}", func),
            );
        }
    }

    if line.contains("printf") {
        if let Some(captures) = SNPRINTF_RE.captures(line) {
            let buffer = captures.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let size = captures.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            if size != "0" && !size.is_empty() {
                linter.error(
                    linenum,
                    Category::RuntimePrintf,
                    3,
                    &format!(
                        "If you can, use sizeof({}) instead of {} as the 2nd arg to snprintf.",
                        buffer, size
                    ),
                );
            }
        }

        if SPRINTF_RE.is_match(line) {
            linter.error(
                linenum,
                Category::RuntimePrintf,
                5,
                "Never use sprintf. Use snprintf instead.",
            );
        }
    }
}

fn check_printf_format(linter: &mut FileLinter, line: &str, linenum: usize) {
    if line.contains("printf") {
        let matches = PRINTF_FORMAT_SET.matches(line);
        if matches.matched(0) {
            linter.error(
                linenum,
                Category::RuntimePrintfFormat,
                3,
                "%q in format strings is deprecated.  Use %ll instead.",
            );
        }

        if matches.matched(1) {
            linter.error(
                linenum,
                Category::RuntimePrintfFormat,
                2,
                "%N$ formats are unconventional.  Try rewriting to avoid them.",
            );
        }
    }

    if !line.contains('\\') || !(line.contains('"') || line.contains('\'')) {
        return;
    }
    let mut escaped = false;
    let mut printf_unescape = false;
    let mut inside_str = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
            if inside_str && matches!(c, '%' | '[' | '(' | '{') {
                printf_unescape = true;
                break;
            }
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' || c == '\'' {
            inside_str = true;
        }
    }

    if printf_unescape {
        linter.error(
            linenum,
            Category::RuntimePrintfFormat,
            3,
            "%, [, (, and { are undefined character escapes.  Unescape them.",
        );
    }
}

fn check_unary_operator_ampersand(linter: &mut FileLinter, line: &str, linenum: usize) {
    if !line.contains("operator") || !line.contains('&') {
        return;
    }
    if UNARY_OPERATOR_AMPERSAND_RE.is_match(line) {
        linter.error(
            linenum,
            Category::RuntimeOperator,
            4,
            "Unary operator& is dangerous.  Do not use it.",
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_c_integer_types(linter: &mut FileLinter, line: &str, raw_line: &str, linenum: usize) {
    // Check elided line first as it's generally more accurate and often shorter.
    if !C_INTEGER_TYPES_AC.is_match(line) {
        return;
    }

    let line = if line.trim().is_empty() {
        let trimmed_raw = raw_line.trim_start();
        if !trimmed_raw.is_empty()
            && !trimmed_raw.starts_with("//")
            && !trimmed_raw.starts_with("/*")
            && !trimmed_raw.starts_with('*')
        {
            raw_line
        } else {
            line
        }
    } else {
        line
    };

    if !C_INTEGER_TYPES_AC.is_match(line) {
        return;
    }
    if string_utils::contains_word(line, "short port") {
        if !string_utils::contains_word(line, "unsigned short port") {
            linter.error(
                linenum,
                Category::RuntimeInt,
                4,
                "Use \"unsigned short\" for ports, not \"short\"",
            );
        }
        return;
    }

    let mut short_idx = None;
    let mut long_idx = None;

    for mat in C_INTEGER_TYPES_AC.find_iter(line) {
        let needle = C_INTEGER_TYPES_NEEDLES[mat.pattern()];
        let start = mat.start();
        if needle == "short" {
            if short_idx.is_none() && find_word_at(line, start, "short").is_some() {
                short_idx = Some(start);
            }
        } else if needle == "long"
            && long_idx.is_none()
            && find_word_at(line, start, "long").is_some()
            && !string_utils::contains_word(line, "long double")
        {
            long_idx = Some(start);
        }
    }

    let ty = match (short_idx, long_idx) {
        (Some(short_idx), Some(long_idx)) if short_idx < long_idx => "short",
        (Some(_), _) => "short",
        (_, Some(_)) => "long",
        _ => {
            return;
        }
    };

    if ty == "short" && string_utils::contains_word(line, "unsigned short port") {
        return;
    }

    linter.error(
        linenum,
        Category::RuntimeInt,
        4,
        &format!("Use int16_t/int64_t/etc, rather than the C type {}", ty),
    );
}

fn find_word_at(s: &str, start: usize, word: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let end = start + word.len();
    let before_ok = start == 0 || !string_utils::is_word_char(bytes[start - 1]);
    let after_ok = end == bytes.len() || !string_utils::is_word_char(bytes[end]);
    if before_ok && after_ok {
        Some(start)
    } else {
        None
    }
}

fn check_non_const_references(linter: &mut FileLinter, line: &str, linenum: usize) {
    if line.trim_start().starts_with('#')
        || line.trim_end().ends_with('\\')
        || line.contains("static_assert")
        || !line.contains('&')
    {
        return;
    }

    let matches = NON_CONST_REF_CHECK_SET.matches(line);
    if matches.matched(0) || !matches.matched(1) {
        return;
    }

    let Some(captures) = ARG_LIST_RE.captures(line) else {
        return;
    };
    let args = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if args.trim().is_empty() {
        return;
    }

    for arg in split_args(args) {
        let normalized = normalize_template_spacing(arg);
        let type_only = strip_trailing_param_name(normalized.as_ref());
        if !type_only.ends_with('&') || type_only.ends_with("&&") {
            continue;
        }

        if is_const_reference(type_only) {
            continue;
        }

        linter.error(
            linenum,
            Category::RuntimeReferences,
            2,
            &format!(
                "Is this a non-const reference? If so, make const or use a pointer: {}",
                normalized
            ),
        );
    }
}

fn check_variable_length_arrays(linter: &mut FileLinter, line: &str, linenum: usize) {
    if !line.contains('[') || !line.contains(']') {
        return;
    }
    let Some(captures) = VLA_DECL_RE.captures(line) else {
        return;
    };

    let leading_token = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if matches!(leading_token, "return" | "delete") {
        return;
    }
    let size_expr = captures.get(2).map(|m| m.as_str().trim()).unwrap_or("");
    if size_expr.is_empty() || size_expr.contains(']') {
        return;
    }

    let mut skip_next = false;
    let tokens = TOKEN_SPLIT_RE.split(size_expr);
    for token in tokens {
        if skip_next {
            skip_next = false;
            continue;
        }

        if token.is_empty() {
            continue;
        }
        if SIZEOF_TOKEN_RE.is_match(token) || ARRAYSIZE_TOKEN_RE.is_match(token) {
            continue;
        }

        let cleaned = token.trim_start_matches('(').trim_end_matches(')');
        if cleaned.is_empty()
            || string_utils::str_is_digit(cleaned)
            || CONSTANT_MATCH_SET.is_match(cleaned)
        {
            continue;
        }
        if cleaned.starts_with("sizeof") {
            skip_next = true;
            continue;
        }

        linter.error(
            linenum,
            Category::RuntimeArrays,
            1,
            "Do not use variable-length arrays.  Use an appropriately named ('k' followed by CamelCase) compile-time constant for the size.",
        );
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::state::CppLintState;
    use std::path::PathBuf;

    fn test_ampersand(line: &str) -> bool {
        let state = CppLintState::new();
        let options = Options::new();
        let mut linter = FileLinter::new(PathBuf::from("test.cpp"), &state, options);
        check_unary_operator_ampersand(&mut linter, line, 1);
        state.error_count() > 0
    }

    #[test]
    fn test_check_unary_operator_ampersand() {
        assert!(test_ampersand("operator&()"));
        assert!(test_ampersand("operator& ()"));
        assert!(test_ampersand("operator & ()"));
        assert!(test_ampersand("operator &()"));
        assert!(test_ampersand("void operator&()"));
        assert!(test_ampersand("operator&() const"));

        assert!(!test_ampersand("operator&(int)"));
        assert!(!test_ampersand("operator&(int x)"));
        assert!(!test_ampersand("operator=(x & y)"));
        assert!(!test_ampersand("operator & (int)"));
        assert!(!test_ampersand("Foo& operator=(const Foo&);"));
    }
}
