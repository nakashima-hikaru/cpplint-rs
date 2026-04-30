# List of Implemented Rules

This is a list of rules currently implemented in `cpplint-rs`. They are listed individually by category.

Autofix column legend:

- `○`: Autofix supported
- `△`: Partial autofix support (only for specific violation patterns)
- `-`: No autofix support

| Rule Category | Family | Phase | Autofix | Summary |
| :--- | :--- | :--- | :---: | :--- |
| `legal/copyright` | copyright | RawSource | - | Verifies that the file contains a copyright boilerplate comment |
| `build/header_guard` | headers | FileStructure | ○ | Verifies the validity of header guards (#ifndef, #define, #endif) |
| `build/include` | headers | FileStructure | △ | Verifies include consistency and checks for duplications |
| `build/include_alpha` | headers | FileStructure | △ | Verifies alphabetical ordering within include sections |
| `build/include_order` | headers | FileStructure | △ | Verifies include priority (C++, C, OS, etc.) |
| `build/include_subdir` | headers | FileStructure | - | Verifies subdirectory specification in include paths |
| `build/include_what_you_use` | headers | FileStructure | △ | Verifies unnecessary includes or missing forward declarations |
| `whitespace/blank_line` | whitespace | Line | ○ | Checks for extra or missing blank lines |
| `whitespace/braces` | whitespace | Line | △ | Checks space and placement around braces ({, }) |
| `whitespace/comma` | whitespace | Line | ○ | Checks for space after commas (,) |
| `whitespace/comments` | whitespace | Line | ○ | Checks space around and inside comments (//, /* */) |
| `whitespace/empty_conditional_body` | whitespace | Line | ○ | Checks for empty block bodies for if/else, etc. |
| `whitespace/empty_if_body` | whitespace | Line | ○ | Checks for empty if statement blocks |
| `whitespace/empty_loop_body` | whitespace | Line | ○ | Checks for empty loop (for, while) blocks |
| `whitespace/end_of_line` | whitespace | Line | ○ | Checks for extra whitespace at the end of lines |
| `whitespace/ending_newline` | whitespace | Finalize | ○ | Verifies that the file ends with a newline |
| `whitespace/forcolon` | whitespace | Line | ○ | Checks space around colons in range-based for loops |
| `whitespace/indent` | whitespace | Line | △ | Verifies correct indentation (e.g., 2 spaces) |
| `whitespace/indent_namespace` | whitespace | Line | ○ | Verifies indentation rules within namespaces |
| `whitespace/line_length` | whitespace | Line | - | Checks line length limits (standard 80 characters) |
| `whitespace/newline` | whitespace | Line | △ | Checks line break characters and placement |
| `whitespace/operators` | whitespace | Line | △ | Checks space around operators |
| `whitespace/parens` | whitespace | Line | △ | Checks space inside parentheses ( ( ) ) and between function names and parentheses |
| `whitespace/semicolon` | whitespace | Line | ○ | Checks for unnecessary space before semicolons (;) |
| `whitespace/tab` | whitespace | Line | ○ | Prohibits tab characters (spaces recommended) |
| `whitespace/todo` | whitespace | Line | △ | Verifies TODO comment format (TODO(username):) |
| `runtime/arrays` | runtime | Line | - | Recommends containers (std::array, etc.) over fixed-length arrays |
| `runtime/casting` | runtime | Line | - | Prohibits C-style casts and recommends C++ casts |
| `runtime/explicit` | runtime | Line | △ | Checks for `explicit` on single-argument constructors |
| `runtime/init` | runtime | Line | - | Checks for proper variable initialization |
| `runtime/int` | runtime | Line | - | Checks for use of types with unclear sizes like `int` |
| `runtime/invalid_increment` | runtime | Line | ○ | Discourages post-increment for iterators, etc. |
| `runtime/member_string_references` | runtime | Line | - | Warns about the danger of holding `std::string` references as members |
| `runtime/memset` | runtime | Line | ○ | Verifies safety regarding the use of `memset` |
| `runtime/operator` | runtime | Line | - | Checks for proper use of operator overloading |
| `runtime/printf` | runtime | Line | △ | Verifies use and format of `printf` series functions |
| `runtime/printf_format` | runtime | Line | △ | Verifies consistency between `printf` format strings and arguments |
| `runtime/references` | runtime | Line | - | Checks for use of non-const reference arguments (pointers recommended) |
| `runtime/string` | runtime | Line | △ | Verifies efficiency and safety regarding the use of `std::string` |
| `runtime/threadsafe_fn` | runtime | Line | - | Identifies use of non-thread-safe functions (strtok, etc.) |
| `runtime/vlog` | runtime | Line | ○ | Verifies use of `VLOG` macros |
| `build/c++11` | readability | Line | - | Verifies C++11 syntax and feature usage |
| `build/c++17` | readability | Line | - | Verifies C++17 syntax and feature usage |
| `build/deprecated` | readability | Line | - | Verifies use of deprecated features |
| `build/endif_comment` | readability | Line | ○ | Verifies corresponding macro name comment after `#endif` |
| `build/explicit_make_pair` | readability | Line | ○ | Suppresses explicit type specification in `std::make_pair` |
| `build/forward_decl` | readability | Line | ○ | Verifies proper use of forward declarations and include replacements |
| `build/namespaces_headers` | readability | Line | - | Prohibits `using namespace` in header files |
| `build/namespaces_literals` | readability | Line | - | Verifies use of literal namespaces |
| `build/namespaces` | readability | Line | - | Verifies proper declaration and ending of namespaces |
| `build/storage_class` | readability | Line | ○ | Verifies placement of storage classes like `static` or `extern` |
| `readability/alt_tokens` | readability | Line | ○ | Verifies use of alternative tokens like `and`/`or`/`not` |
| `readability/braces` | readability | Line | △ | Verifies opening and closing brace style for blocks |
| `readability/casting` | readability | Line | △ | Verifies readability of casts |
| `readability/check` | readability | Line | ○ | Verifies use of `CHECK` macros |
| `readability/constructors` | readability | Line | - | Verifies constructor declaration and initializer list readability |
| `readability/fn_size` | readability | Line | - | Checks for excessive function line count (complexity) |
| `readability/inheritance` | readability | Line | △ | Verifies inheritance style (virtual/override) |
| `readability/multiline_comment` | readability | Line | - | Verifies format of multi-line comments |
| `readability/multiline_string` | readability | Line | - | Verifies placement of multi-line string literals |
| `readability/namespace` | readability | Line | ○ | Verifies comments at namespace closing positions |
| `readability/nolint` | readability | Line | - | Verifies proper `// NOLINT` usage |
| `readability/nul` | readability | Line | - | Checks for NUL character byte inclusion |
| `readability/strings` | readability | Line | △ | Verifies placement and concatenation of string literals |
| `readability/todo` | readability | Line | - | Verifies readability of TODO comments |
| `readability/utf8` | readability | Line | - | Verifies UTF-8 encoding of source code |
