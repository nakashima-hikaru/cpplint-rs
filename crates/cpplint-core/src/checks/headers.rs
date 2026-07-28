use crate::c_headers;
use crate::categories::Category;
use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use crate::iwyu::IwyuHeader;
use crate::options::IncludeOrder;
use crate::registry::ActiveRulePlan;
use crate::state::{IncludeKind, IncludeState};
use crate::string_utils;
use aho_corasick::AhoCorasick;
use rustc_hash::FxHashSet;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IwyuToken {
    File,
    Allocator,
    BinaryFunction,
    BinaryNegate,
    Bind1st,
    Bind2nd,
    Cerr,
    CharTraits,
    Cin,
    Clearerr,
    Clog,
    ConstMemFun1RefT,
    ConstMemFun1T,
    ConstMemFunRefT,
    ConstMemFunT,
    Copy,
    Cout,
    Divides,
    EqualTo,
    Fclose,
    Feof,
    Ferror,
    Fflush,
    Fgetc,
    Fgetpos,
    Fgets,
    Fopen,
    Forward,
    FposT,
    Fprintf,
    Fputc,
    Fputs,
    Fread,
    Freopen,
    Fscanf,
    Fseek,
    Fsetpos,
    Ftell,
    Fwrite,
    Getc,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    List,
    LogicalAnd,
    LogicalNot,
    LogicalOr,
    MakePair,
    MakeShared,
    MakeUnique,
    Map,
    Max,
    MemFun,
    MemFun1RefT,
    MemFun1T,
    MemFunRef,
    MemFunRefT,
    MemFunT,
    Min,
    MinElement,
    Minus,
    Modulus,
    Move,
    Multimap,
    Multiplies,
    Negate,
    Not1,
    Not2,
    NotEqualTo,
    NumericLimits,
    Pair,
    Perror,
    Plus,
    PointerToBinaryFunction,
    PointerToUnaryFunction,
    Printf,
    PtrFun,
    Putc,
    Putchar,
    Puts,
    Scanf,
    Set,
    Setbuf,
    Setvbuf,
    SharedPtr,
    Snprintf,
    Sort,
    Sprintf,
    Sscanf,
    String,
    Swap,
    Tmpnam,
    Transform,
    Tuple,
    UnaryFunction,
    UnaryNegate,
    Ungetc,
    UniquePtr,
    Vector,
    Vfprintf,
    Vfscanf,
    Vprintf,
    Vscanf,
    Vsnprintf,
    Vsscanf,
    Wcerr,
    Wcin,
    Wclog,
    Wcout,
    WeakPtr,
}

impl IwyuToken {
    fn as_str(&self) -> &'static str {
        match self {
            IwyuToken::File => "FILE",
            IwyuToken::Allocator => "allocator",
            IwyuToken::BinaryFunction => "binary_function",
            IwyuToken::BinaryNegate => "binary_negate",
            IwyuToken::Bind1st => "bind1st",
            IwyuToken::Bind2nd => "bind2nd",
            IwyuToken::Cerr => "cerr",
            IwyuToken::CharTraits => "char_traits",
            IwyuToken::Cin => "cin",
            IwyuToken::Clearerr => "clearerr",
            IwyuToken::Clog => "clog",
            IwyuToken::ConstMemFun1RefT => "const_mem_fun1_ref_t",
            IwyuToken::ConstMemFun1T => "const_mem_fun1_t",
            IwyuToken::ConstMemFunRefT => "const_mem_fun_ref_t",
            IwyuToken::ConstMemFunT => "const_mem_fun_t",
            IwyuToken::Copy => "copy",
            IwyuToken::Cout => "cout",
            IwyuToken::Divides => "divides",
            IwyuToken::EqualTo => "equal_to",
            IwyuToken::Fclose => "fclose",
            IwyuToken::Feof => "feof",
            IwyuToken::Ferror => "ferror",
            IwyuToken::Fflush => "fflush",
            IwyuToken::Fgetc => "fgetc",
            IwyuToken::Fgetpos => "fgetpos",
            IwyuToken::Fgets => "fgets",
            IwyuToken::Fopen => "fopen",
            IwyuToken::Forward => "forward",
            IwyuToken::FposT => "fpos_t",
            IwyuToken::Fprintf => "fprintf",
            IwyuToken::Fputc => "fputc",
            IwyuToken::Fputs => "fputs",
            IwyuToken::Fread => "fread",
            IwyuToken::Freopen => "freopen",
            IwyuToken::Fscanf => "fscanf",
            IwyuToken::Fseek => "fseek",
            IwyuToken::Fsetpos => "fsetpos",
            IwyuToken::Ftell => "ftell",
            IwyuToken::Fwrite => "fwrite",
            IwyuToken::Getc => "getc",
            IwyuToken::Greater => "greater",
            IwyuToken::GreaterEqual => "greater_equal",
            IwyuToken::Less => "less",
            IwyuToken::LessEqual => "less_equal",
            IwyuToken::List => "list",
            IwyuToken::LogicalAnd => "logical_and",
            IwyuToken::LogicalNot => "logical_not",
            IwyuToken::LogicalOr => "logical_or",
            IwyuToken::MakePair => "make_pair",
            IwyuToken::MakeShared => "make_shared",
            IwyuToken::MakeUnique => "make_unique",
            IwyuToken::Map => "map",
            IwyuToken::Max => "max",
            IwyuToken::MemFun => "mem_fun",
            IwyuToken::MemFun1RefT => "mem_fun1_ref_t",
            IwyuToken::MemFun1T => "mem_fun1_t",
            IwyuToken::MemFunRef => "mem_fun_ref",
            IwyuToken::MemFunRefT => "mem_fun_ref_t",
            IwyuToken::MemFunT => "mem_fun_t",
            IwyuToken::Min => "min",
            IwyuToken::MinElement => "min_element",
            IwyuToken::Minus => "minus",
            IwyuToken::Modulus => "modulus",
            IwyuToken::Move => "move",
            IwyuToken::Multimap => "multimap",
            IwyuToken::Multiplies => "multiplies",
            IwyuToken::Negate => "negate",
            IwyuToken::Not1 => "not1",
            IwyuToken::Not2 => "not2",
            IwyuToken::NotEqualTo => "not_equal_to",
            IwyuToken::NumericLimits => "numeric_limits",
            IwyuToken::Pair => "pair",
            IwyuToken::Perror => "perror",
            IwyuToken::Plus => "plus",
            IwyuToken::PointerToBinaryFunction => "pointer_to_binary_function",
            IwyuToken::PointerToUnaryFunction => "pointer_to_unary_function",
            IwyuToken::Printf => "printf",
            IwyuToken::PtrFun => "ptr_fun",
            IwyuToken::Putc => "putc",
            IwyuToken::Putchar => "putchar",
            IwyuToken::Puts => "puts",
            IwyuToken::Scanf => "scanf",
            IwyuToken::Set => "set",
            IwyuToken::Setbuf => "setbuf",
            IwyuToken::Setvbuf => "setvbuf",
            IwyuToken::SharedPtr => "shared_ptr",
            IwyuToken::Snprintf => "snprintf",
            IwyuToken::Sort => "sort",
            IwyuToken::Sprintf => "sprintf",
            IwyuToken::Sscanf => "sscanf",
            IwyuToken::String => "string",
            IwyuToken::Swap => "swap",
            IwyuToken::Tmpnam => "tmpnam",
            IwyuToken::Transform => "transform",
            IwyuToken::Tuple => "tuple",
            IwyuToken::UnaryFunction => "unary_function",
            IwyuToken::UnaryNegate => "unary_negate",
            IwyuToken::Ungetc => "ungetc",
            IwyuToken::UniquePtr => "unique_ptr",
            IwyuToken::Vector => "vector",
            IwyuToken::Vfprintf => "vfprintf",
            IwyuToken::Vfscanf => "vfscanf",
            IwyuToken::Vprintf => "vprintf",
            IwyuToken::Vscanf => "vscanf",
            IwyuToken::Vsnprintf => "vsnprintf",
            IwyuToken::Vsscanf => "vsscanf",
            IwyuToken::Wcerr => "wcerr",
            IwyuToken::Wcin => "wcin",
            IwyuToken::Wclog => "wclog",
            IwyuToken::Wcout => "wcout",
            IwyuToken::WeakPtr => "weak_ptr",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IwyuCheck {
    Word {
        token: IwyuToken,
        header: IwyuHeader,
    },
    FuncOrTempl {
        token: IwyuToken,
        header: IwyuHeader,
    },
    StdTempl {
        token: IwyuToken,
        header: IwyuHeader,
    },
    Templ {
        token: IwyuToken,
        header: IwyuHeader,
    },
    Func {
        token: IwyuToken,
        header: IwyuHeader,
    },
}

impl IwyuCheck {
    fn token(&self) -> IwyuToken {
        match *self {
            IwyuCheck::Word { token, .. } => token,
            IwyuCheck::FuncOrTempl { token, .. } => token,
            IwyuCheck::StdTempl { token, .. } => token,
            IwyuCheck::Templ { token, .. } => token,
            IwyuCheck::Func { token, .. } => token,
        }
    }

    fn header(&self) -> IwyuHeader {
        match *self {
            IwyuCheck::Word { header, .. } => header,
            IwyuCheck::FuncOrTempl { header, .. } => header,
            IwyuCheck::StdTempl { header, .. } => header,
            IwyuCheck::Templ { header, .. } => header,
            IwyuCheck::Func { header, .. } => header,
        }
    }
}

const IWYU_CHECKS: &[IwyuCheck] = &[
    IwyuCheck::Word {
        token: IwyuToken::String,
        header: IwyuHeader::String,
    },
    IwyuCheck::Word {
        token: IwyuToken::Cin,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Cout,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Cerr,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Clog,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Wcin,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Wcout,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Wcerr,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::Wclog,
        header: IwyuHeader::Iostream,
    },
    IwyuCheck::Word {
        token: IwyuToken::File,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Word {
        token: IwyuToken::FposT,
        header: IwyuHeader::Cstdio,
    },
    // Algorithm
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Copy,
        header: IwyuHeader::Algorithm,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Max,
        header: IwyuHeader::Algorithm,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Min,
        header: IwyuHeader::Algorithm,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::MinElement,
        header: IwyuHeader::Algorithm,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Sort,
        header: IwyuHeader::Algorithm,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Transform,
        header: IwyuHeader::Algorithm,
    },
    // Utility
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Forward,
        header: IwyuHeader::Utility,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::MakePair,
        header: IwyuHeader::Utility,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Move,
        header: IwyuHeader::Utility,
    },
    IwyuCheck::FuncOrTempl {
        token: IwyuToken::Swap,
        header: IwyuHeader::Utility,
    },
    // Map
    IwyuCheck::StdTempl {
        token: IwyuToken::Map,
        header: IwyuHeader::Map,
    },
    // Templates
    IwyuCheck::Templ {
        token: IwyuToken::UnaryFunction,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::BinaryFunction,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Plus,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Minus,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Multiplies,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Divides,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Modulus,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Negate,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::EqualTo,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::NotEqualTo,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Greater,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Less,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::GreaterEqual,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::LessEqual,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::LogicalAnd,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::LogicalOr,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::LogicalNot,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::UnaryNegate,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Not1,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::BinaryNegate,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Not2,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Bind1st,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Bind2nd,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::PointerToUnaryFunction,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::PointerToBinaryFunction,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::PtrFun,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFunT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFun,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFun1T,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFun1RefT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFunRefT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::ConstMemFunT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::ConstMemFun1T,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::ConstMemFunRefT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::ConstMemFun1RefT,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MemFunRef,
        header: IwyuHeader::Functional,
    },
    IwyuCheck::Templ {
        token: IwyuToken::List,
        header: IwyuHeader::List,
    },
    IwyuCheck::Templ {
        token: IwyuToken::NumericLimits,
        header: IwyuHeader::Limits,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Multimap,
        header: IwyuHeader::Map,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Allocator,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MakeShared,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::MakeUnique,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::SharedPtr,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::UniquePtr,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::WeakPtr,
        header: IwyuHeader::Memory,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Set,
        header: IwyuHeader::Set,
    },
    IwyuCheck::Templ {
        token: IwyuToken::CharTraits,
        header: IwyuHeader::String,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Tuple,
        header: IwyuHeader::Tuple,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Pair,
        header: IwyuHeader::Utility,
    },
    IwyuCheck::Templ {
        token: IwyuToken::Vector,
        header: IwyuHeader::Vector,
    },
    // cstdio functions
    IwyuCheck::Func {
        token: IwyuToken::Fgets,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fclose,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Clearerr,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Feof,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Ferror,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fflush,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fgetpos,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fread,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fgetc,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fputc,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fputs,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fopen,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Freopen,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fseek,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fsetpos,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Ftell,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Getc,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Putc,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Putchar,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Perror,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Printf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Puts,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Scanf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Setbuf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Setvbuf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Snprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Sprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Sscanf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Tmpnam,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Ungetc,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vfprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vfscanf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vsnprintf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vscanf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Vsscanf,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fwrite,
        header: IwyuHeader::Cstdio,
    },
    IwyuCheck::Func {
        token: IwyuToken::Fscanf,
        header: IwyuHeader::Cstdio,
    },
];

static IWYU_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let patterns: Vec<&str> = IWYU_CHECKS.iter().map(|c| c.token().as_str()).collect();
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

pub fn check_header_guard(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>) {
    let extension = Path::new(linter.filename())
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if !linter.options().is_header_extension(extension) {
        return;
    }

    let raw_lines = &clean_lines.lines_without_raw_strings;

    // Respect the documented file-level suppression for synthetic guard errors.
    for line in raw_lines {
        if line.contains("NOLINT") && NOLINT_HEADER_GUARD_RE.is_match(line) {
            return;
        }
    }

    // 1. Check for #pragma once
    for line in raw_lines {
        if line.contains("pragma") && PRAGMA_ONCE_RE.is_match(line) {
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
        && guard == d_guard
    {
        if guard != expected_guard {
            linter.error(
                line_idx,
                Category::BuildHeaderGuard,
                5,
                crate::messages::LintMessage::HeaderGuardWrongStyle(expected_guard.as_str().into()),
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
                Category::BuildHeaderGuard,
                0,
                crate::messages::LintMessage::EndifLineShouldBe(expected_slash.into()),
            );
            return;
        }

        if endif_line == expected_block_legacy {
            linter.error(
                endif_idx,
                Category::BuildHeaderGuard,
                0,
                crate::messages::LintMessage::EndifLineShouldBe(expected_block.into()),
            );
            return;
        }

        linter.error(
            endif_idx,
            Category::BuildHeaderGuard,
            5,
            crate::messages::LintMessage::EndifLineShouldBe(expected_slash.into()),
        );
        return;
    }

    linter.error_display_line(
        0,
        Category::BuildHeaderGuard,
        5,
        crate::messages::LintMessage::HeaderGuardMissingSuggested(expected_guard.into()),
    );
}

pub fn check_includes(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    active_rules: ActiveRulePlan,
) {
    let mut include_state = IncludeState::new();
    let options = linter.options_arc();
    let header_extensions = options.header_extensions();
    let non_header_extensions = options.non_header_extensions();
    let file_from_repo = linter.relative_from_repository_arc();
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

    let check_subdir = active_rules.is_enabled(Category::BuildIncludeSubdir);
    let check_cpp11 = active_rules.is_enabled(Category::BuildCpp11);
    let check_cpp17 = active_rules.is_enabled(Category::BuildCpp17);
    let check_include = active_rules.is_enabled(Category::BuildInclude);
    let check_order = active_rules.is_enabled(Category::BuildIncludeOrder);
    let check_alpha = active_rules.is_enabled(Category::BuildIncludeAlpha);

    for (linenum, line) in clean_lines.lines_without_raw_strings.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !trimmed.starts_with('#') || !trimmed.contains("include") {
            if trimmed.starts_with('#')
                && let Some(directive) = preprocessor_directive(trimmed)
            {
                include_state.reset_section(directive);
            }
            continue;
        }

        let Some((delim, include)) = string_utils::parse_include_directive(trimmed) else {
            if let Some(directive) = preprocessor_directive(trimmed) {
                include_state.reset_section(directive);
            }
            continue;
        };

        let used_angle_brackets = delim == "<";
        let kind = classify_include(
            &file_from_repo,
            Path::new(include),
            used_angle_brackets,
            linter.options().include_order,
        );
        if check_subdir
            && delim == "\""
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
                Category::BuildIncludeSubdir,
                4,
                crate::messages::LintMessage::IncludeDirectoryWhenNamingHeaderFiles,
            );
        }

        if check_cpp11 && matches!(include, "cfenv" | "fenv.h" | "ratio") {
            linter.error(
                linenum,
                Category::BuildCpp11,
                5,
                crate::messages::LintMessage::UnapprovedCpp11Header(include.into()),
            );
        }

        if check_cpp17 && include == "filesystem" {
            linter.error(
                linenum,
                Category::BuildCpp17,
                5,
                crate::messages::LintMessage::UnapprovedCpp17FilesystemHeader,
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
            if check_include {
                linter.error(
                    linenum,
                    Category::BuildInclude,
                    4,
                    crate::messages::LintMessage::AlreadyIncluded(
                        include.to_string().into(),
                        linter.filename().to_string().into(),
                        first_line + 1,
                    ),
                );
            }
            continue;
        }

        // ⚡ Bolt: Avoid format!() allocation in hot loop by using strip_suffix and ends_with
        let includes_non_header_from_other_package =
            non_header_extensions.iter().find(|extension| {
                include
                    .strip_suffix(extension.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
                    && file_from_repo_dir
                        != Path::new(include).parent().unwrap_or_else(|| Path::new(""))
            });
        if let Some(extension) = includes_non_header_from_other_package {
            if check_include {
                linter.error(
                    linenum,
                    Category::BuildInclude,
                    4,
                    crate::messages::LintMessage::DoNotIncludeExtensionFromOtherPackages(
                        extension.to_string().into(),
                    ),
                );
            }
            continue;
        }

        let include_has_alias = include.contains("./") || include.contains("../");
        let third_src_header = if include_has_alias {
            false
        } else {
            // ⚡ Bolt: Avoid format!() allocation in hot loop by reusing a pre-allocated string
            let mut headername = String::with_capacity(basefilename_relative.len() + 8);
            headername.push_str(&basefilename_relative);
            headername.push('.');
            let base_len = headername.len();
            header_extensions.iter().any(|ext| {
                headername.truncate(base_len);
                headername.push_str(ext);
                headername.contains(include) || include.contains(&headername)
            })
        };
        if third_src_header || !is_special_include_name(include) {
            include_state.push_include(include, linenum);
            if let Some(message) = include_state.check_next_include_order(kind)
                && check_order
            {
                let basename = Path::new(linter.filename())
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("");
                linter.error(
                    linenum,
                    Category::BuildIncludeOrder,
                    4,
                    crate::messages::LintMessage::IncludeOrder(
                        message.to_string().into(),
                        basename.to_string().into(),
                    ),
                );
            }

            let canonical_include = include_state.canonicalize_alphabetical_order(include);
            let prev_elided = if linenum > 0 {
                clean_lines.elided[linenum - 1].trim()
            } else {
                ""
            };
            let previous_line_is_include = linenum > 0
                && prev_elided.starts_with('#')
                && prev_elided.contains("include")
                && string_utils::parse_include_directive(prev_elided).is_some();
            if check_alpha
                && !include_state
                    .is_in_alphabetical_order(previous_line_is_include, &canonical_include)
            {
                linter.error(
                    linenum,
                    Category::BuildIncludeAlpha,
                    4,
                    crate::messages::LintMessage::IncludeAlpha(include.to_string().into()),
                );
            }
            include_state.set_last_header(&canonical_include);
        }
    }

    if active_rules.is_enabled(Category::BuildIncludeWhatYouUse) {
        check_include_what_you_use(linter, clean_lines, &include_state);
    }
    if active_rules.is_enabled(Category::BuildInclude) {
        check_header_file_included(linter, &include_state);
    }
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
        .unwrap_or("");
    let is_system = used_angle_brackets && !matches!(include_ext, "hh" | "hpp" | "hxx" | "h++");
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
    let target_base = target_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let include_file = drop_common_suffixes(include);
    let include_base = include_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if files_belong_to_same_module(path_from_repo, include).0 {
        return IncludeKind::LikelyMyHeader;
    }

    if has_alias_component(include) {
        return IncludeKind::OtherHeader;
    }

    if first_component(target_base) == first_component(include_base) {
        return IncludeKind::PossibleMyHeader;
    }

    IncludeKind::OtherHeader
}

fn is_non_header_extension(ext: &str) -> bool {
    matches!(ext, "c" | "cc" | "cpp" | "cxx" | "c++" | "cu")
}

fn is_header_extension(ext: &str) -> bool {
    matches!(ext, "h" | "hh" | "hpp" | "hxx" | "h++" | "cuh")
}

fn strip_test_suffix(stem: &str) -> &str {
    for suffix in ["_unittest", "_regtest", "_test"] {
        if let Some(stripped) = stem.strip_suffix(suffix) {
            return stripped;
        }
    }
    stem
}

fn normalize_module_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace("/public/", "/")
        .replace("/internal/", "/")
}

fn path_without_extension(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        // ⚡ Bolt: Avoid format!(".{ext}") allocation by directly using lengths
        let ext_len_with_dot = ext.len() + 1;
        if value.len() >= ext_len_with_dot
            && value[value.len() - ext_len_with_dot..].starts_with('.')
        {
            value.truncate(value.len() - ext_len_with_dot);
        }
    }
    value
}

fn files_belong_to_same_module(filename_cc: &Path, filename_h: &Path) -> (bool, String) {
    let cc_ext = filename_cc
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if !is_non_header_extension(cc_ext) {
        return (false, String::new());
    }
    let h_ext = filename_h
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if !is_header_extension(h_ext) {
        return (false, String::new());
    }

    let cc_no_ext = path_without_extension(filename_cc);
    let cc_stem = Path::new(&cc_no_ext)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let cc_module_stem = strip_test_suffix(cc_stem);
    let cc_prefix_len = cc_no_ext.len().saturating_sub(cc_stem.len());
    let mut cc_module = format!("{}{}", &cc_no_ext[..cc_prefix_len], cc_module_stem);
    cc_module = normalize_module_path(Path::new(&cc_module));

    let mut h_module = path_without_extension(filename_h);
    if let Some(stripped) = h_module.strip_suffix("-inl") {
        h_module = stripped.to_string();
    }
    h_module = normalize_module_path(Path::new(&h_module));

    let belongs = cc_module.ends_with(&h_module);
    if !belongs {
        return (false, String::new());
    }
    let common = cc_module
        .strip_suffix(&h_module)
        .unwrap_or_default()
        .to_string();
    (true, common)
}

fn preprocessor_directive(trimmed: &str) -> Option<&str> {
    let directive = trimmed.strip_prefix('#')?.trim_start();
    ["if", "ifdef", "ifndef", "else", "elif", "endif"]
        .into_iter()
        .find(|candidate| directive.starts_with(candidate))
}

fn is_special_include_name(include: &str) -> bool {
    if SPECIAL_INCLUDE_AC.is_match(include) {
        return true;
    }
    include.ends_with(".h")
        && !include.contains('/')
        && include.bytes().any(|b| b.is_ascii_uppercase())
}

fn drop_common_suffixes(path: &Path) -> PathBuf {
    let value = path.to_string_lossy().replace('\\', "/");
    let mut base = value.as_str();
    for ext in [
        ".h", ".hh", ".hpp", ".hxx", ".h++", ".c", ".cc", ".cpp", ".cxx", ".c++",
    ] {
        if let Some(stripped) = base.strip_suffix(ext) {
            base = stripped;
            break;
        }
    }
    for suffix in ["-inl", "_inl", "_unittest", "_regtest", "_test"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped;
            break;
        }
    }
    PathBuf::from(base)
}

fn has_alias_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

#[cfg(test)]
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
    clean_lines: &CleansedLines<'_>,
    include_state: &IncludeState,
) {
    let mut required: BTreeMap<IwyuHeader, (usize, String)> = BTreeMap::new();

    for (linenum, line) in clean_lines.elided.iter().enumerate() {
        if clean_lines.raw_lines[linenum].contains("NOLINT") {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut matched_headers = FxHashSet::default();
        for mat in IWYU_AC.find_iter(line) {
            let start = mat.start();
            let end = mat.end();
            let check = IWYU_CHECKS[mat.pattern()];
            let header = check.header();
            if matched_headers.contains(&header) {
                continue;
            }

            let m = IwyuMatch { line, start, end };
            match check {
                IwyuCheck::Word { token, .. } => {
                    if m.is_word_match() {
                        required.insert(header, (linenum, token.as_str().to_string()));
                        matched_headers.insert(header);
                    }
                }
                IwyuCheck::FuncOrTempl { token, .. } => {
                    if m.is_function_or_template_match() {
                        required.insert(header, (linenum, token.as_str().to_string()));
                        matched_headers.insert(header);
                    }
                }
                IwyuCheck::StdTempl { token, .. } => {
                    if m.is_std_template_match() {
                        required.insert(header, (linenum, format!("{}<>", token.as_str())));
                        matched_headers.insert(header);
                    }
                }
                IwyuCheck::Templ { token, .. } => {
                    if m.is_template_match() {
                        required.insert(header, (linenum, format!("{}<>", token.as_str())));
                        matched_headers.insert(header);
                    }
                }
                IwyuCheck::Func { token, .. } => {
                    if m.is_function_match() {
                        required.insert(header, (linenum, token.as_str().to_string()));
                        matched_headers.insert(header);
                    }
                }
            }
        }
    }

    for (header, (linenum, symbol)) in required {
        if include_state.find_header(header.as_str()).is_none() {
            linter.error(
                linenum,
                Category::BuildIncludeWhatYouUse,
                4,
                crate::messages::LintMessage::IwyuAddInclude(header, symbol.into()),
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
    if linter.options().is_header_extension(extension) {
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
    let file_from_repo = linter.relative_from_repository_arc();
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

    let options = linter.options_arc();
    for header_ext in options.header_extensions() {
        let header_path = directory.join(format!("{}.{}", stem, header_ext));
        if !header_path.is_file() {
            continue;
        }

        let mut header_name = file_from_repo
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("{}.{}", stem, header_ext))
            .to_string_lossy()
            .replace('\\', "/");
        if header_name.is_empty() {
            header_name = format!("{}.{}", stem, header_ext);
        }

        let found = include_state.include_lists().iter().any(|section_list| {
            section_list.iter().any(|(include, _)| {
                !has_alias_component(Path::new(include))
                    && (header_name.contains(include) || include.contains(&header_name))
            })
        });
        if found {
            return;
        }

        linter.error(
            first_include_line.unwrap_or(0),
            Category::BuildInclude,
            5,
            crate::messages::LintMessage::MissingSelfHeader {
                file_from_repo: path_from_repo.into(),
                header: header_name.into(),
                includes_use_aliases,
            },
        );
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn helper_functions_cover_include_classification_and_normalization() {
        assert_eq!(preprocessor_directive("#ifdef FOO"), Some("if"));
        assert_eq!(preprocessor_directive("#elif defined(FOO)"), Some("elif"));
        assert_eq!(preprocessor_directive("not a directive"), None);

        assert!(is_special_include_name("lua.h"));
        assert!(is_special_include_name("Foo.h"));
        assert!(!is_special_include_name("foo.cc"));

        assert_eq!(
            drop_common_suffixes(Path::new("src/foo-inl.h")),
            PathBuf::from("src/foo")
        );
        assert_eq!(
            normalize_path(Path::new("src/./include/../foo")),
            PathBuf::from("src/foo")
        );
        assert_eq!(first_component("foo-bar.baz"), "foo");
        assert_eq!(generate_guard(Path::new("foo/bar-baz.h")), "FOO_BAR_BAZ_H_");

        assert_eq!(
            classify_include(
                Path::new("src/foo/bar.cc"),
                Path::new("<vector>"),
                true,
                IncludeOrder::Default,
            ),
            IncludeKind::CSystem
        );
        assert_eq!(
            classify_include(
                Path::new("src/foo/bar.cc"),
                Path::new("src/foo/bar.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::LikelyMyHeader
        );
        assert_eq!(
            classify_include(
                Path::new("src/foo/bar.cc"),
                Path::new("src/foo/bar_utils.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::PossibleMyHeader
        );
        assert_eq!(
            classify_include(
                Path::new("src/foo/bar.cc"),
                Path::new("third_party/other.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::OtherHeader
        );
    }

    #[test]
    fn iwyu_match_helpers_cover_word_function_and_template_paths() {
        let word = IwyuMatch {
            line: "std::vector value;",
            start: 5,
            end: 11,
        };
        assert!(word.is_word_match());

        let function = IwyuMatch {
            line: "std::make_pair(1, 2)",
            start: 5,
            end: 14,
        };
        assert!(function.is_function_match());

        let templ = IwyuMatch {
            line: "std::vector<int> values;",
            start: 5,
            end: 11,
        };
        assert!(templ.is_std_template_match());
        assert!(templ.is_template_match());

        let func_or_template = IwyuMatch {
            line: "std::vector<int>(1)",
            start: 5,
            end: 11,
        };
        assert!(func_or_template.is_function_or_template_match());
    }

    #[test]
    fn test_files_belong_to_same_module() {
        let f = |cc: &str, h: &str| files_belong_to_same_module(Path::new(cc), Path::new(h));

        assert_eq!(f("a.cc", "a.h"), (true, "".to_string()));
        assert_eq!(f("base/google.cc", "base/google.h"), (true, "".to_string()));
        assert_eq!(
            f("base/google_test.c", "base/google.h"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/google_test.cc", "base/google.hpp"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/google_test.cxx", "base/google.hxx"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/google_test.c++", "base/google.h++"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/google_test.cu", "base/google.cuh"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/google_unittest.cc", "base/google-inl.h"),
            (true, "".to_string())
        );
        assert_eq!(
            f("base/internal/google_unittest.cc", "base/public/google.h"),
            (true, "".to_string())
        );
        assert_eq!(
            f(
                "xxx/yyy/base/internal/google_unittest.cc",
                "base/public/google.h"
            ),
            (true, "xxx/yyy/".to_string())
        );
        assert_eq!(
            f("xxx/yyy/base/google_unittest.cc", "base/public/google.h"),
            (true, "xxx/yyy/".to_string())
        );
        assert_eq!(
            f("/home/build/google3/base/google.cc", "base/google.h"),
            (true, "/home/build/google3/".to_string())
        );

        assert_eq!(
            f("/home/build/google3/base/google.cc", "basu/google.h"),
            (false, "".to_string())
        );
        assert_eq!(f("a.cc", "b.h"), (false, "".to_string()));
    }

    #[test]
    fn test_classify_include() {
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("stdio.h"),
                true,
                IncludeOrder::Default,
            ),
            IncludeKind::CSystem
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("string"),
                true,
                IncludeOrder::Default,
            ),
            IncludeKind::CppSystem
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("foo/foo.h"),
                true,
                IncludeOrder::Default,
            ),
            IncludeKind::CSystem
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("foo/foo.h"),
                true,
                IncludeOrder::StandardCFirst,
            ),
            IncludeKind::OtherSystem
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("string"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::OtherHeader
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("boost/any.hpp"),
                true,
                IncludeOrder::Default,
            ),
            IncludeKind::OtherHeader
        );
        assert_eq!(
            classify_include(
                Path::new("foo/foo.cc"),
                Path::new("foo/foo-inl.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::LikelyMyHeader
        );
        assert_eq!(
            classify_include(
                Path::new("foo/internal/foo.cc"),
                Path::new("foo/public/foo.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::LikelyMyHeader
        );
        assert_eq!(
            classify_include(
                Path::new("foo/internal/foo.cc"),
                Path::new("foo/other/public/foo.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::PossibleMyHeader
        );
        assert_eq!(
            classify_include(
                Path::new("foo/internal/foo.cc"),
                Path::new("foo/other/public/foop.h"),
                false,
                IncludeOrder::Default,
            ),
            IncludeKind::OtherHeader
        );
    }

    #[test]
    fn test_try_drop_common_suffixes() {
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo-inl.h")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo-inl.hxx")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo-inl.h++")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo-inl.hpp")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/bar/foo_inl.h")),
            PathBuf::from("foo/bar/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo.cc")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo.cxx")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo.c")),
            PathBuf::from("foo/foo")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("foo/foo_unusualinternal.h")),
            PathBuf::from("foo/foo_unusualinternal")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("_test.cc")),
            PathBuf::from("")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("test.cc")),
            PathBuf::from("test")
        );
        assert_eq!(
            drop_common_suffixes(Path::new("test.c++")),
            PathBuf::from("test")
        );
    }
}
