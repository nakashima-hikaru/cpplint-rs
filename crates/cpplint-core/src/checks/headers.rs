use crate::c_headers;
use crate::categories::Category;
use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use crate::options::IncludeOrder;
use crate::state::{IncludeKind, IncludeState};
use aho_corasick::AhoCorasick;
use fxhash::FxHashSet;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static INCLUDE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^\s*#\s*include\s*([<"])([^>"]+)[>"]"#).unwrap());

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum IwyuHeader {
    Algorithm,
    Cstdio,
    Functional,
    Iostream,
    Limits,
    List,
    Map,
    Memory,
    Set,
    String,
    Tuple,
    Utility,
    Vector,
}

impl IwyuHeader {
    fn as_str(&self) -> &'static str {
        match self {
            IwyuHeader::Algorithm => "algorithm",
            IwyuHeader::Cstdio => "cstdio",
            IwyuHeader::Functional => "functional",
            IwyuHeader::Iostream => "iostream",
            IwyuHeader::Limits => "limits",
            IwyuHeader::List => "list",
            IwyuHeader::Map => "map",
            IwyuHeader::Memory => "memory",
            IwyuHeader::Set => "set",
            IwyuHeader::String => "string",
            IwyuHeader::Tuple => "tuple",
            IwyuHeader::Utility => "utility",
            IwyuHeader::Vector => "vector",
        }
    }
}

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
        && guard == d_guard
    {
        if guard != expected_guard {
            linter.error(
                line_idx,
                Category::BuildHeaderGuard,
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
                Category::BuildHeaderGuard,
                0,
                &format!(r#"#endif line should be "{}""#, expected_slash),
            );
            return;
        }

        if endif_line == expected_block_legacy {
            linter.error(
                endif_idx,
                Category::BuildHeaderGuard,
                0,
                &format!(r#"#endif line should be "{}""#, expected_block),
            );
            return;
        }

        linter.error(
            endif_idx,
            Category::BuildHeaderGuard,
            5,
            &format!(r#"#endif line should be "{}""#, expected_slash),
        );
        return;
    }

    linter.error_display_line(
        0,
        Category::BuildHeaderGuard,
        5,
        &format!(
            "No #ifndef header guard found, suggested CPP variable is: {}",
            expected_guard
        ),
    );
}

pub fn check_includes(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>) {
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
                Category::BuildIncludeSubdir,
                4,
                "Include the directory when naming header files",
            );
        }

        if matches!(include, "cfenv" | "fenv.h" | "ratio") {
            linter.error(
                linenum,
                Category::BuildCpp11,
                5,
                &format!("<{}> is an unapproved C++11 header.", include),
            );
        }

        if include == "filesystem" {
            linter.error(
                linenum,
                Category::BuildCpp17,
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
                Category::BuildInclude,
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
                Category::BuildInclude,
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
                    Category::BuildIncludeOrder,
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
                    Category::BuildIncludeAlpha,
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
                &format!("Add #include <{}> for {}", header.as_str(), symbol),
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
            Category::BuildInclude,
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
