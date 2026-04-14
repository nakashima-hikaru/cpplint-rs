macro_rules! define_c_standard_header_folders {
    ($($folder:literal),* $(,)?) => {
        pub const C_STANDARD_HEADER_FOLDERS: &[&str] = &[
            $($folder),*
        ];

        define_c_standard_header_folders!(@build_pattern [] $($folder),*);
    };
    
    (@build_pattern [$($acc:expr),*] $last:literal) => {
        pub const HEADER_FOLDERS_PATTERN: &str = concat!("(?:", $($acc,)* $last, r")\/.*\.h");
        
        pub fn get_header_folders_pattern() -> &'static str {
            HEADER_FOLDERS_PATTERN
        }
    };
    
    (@build_pattern [$($acc:expr),*] $first:literal, $($rest:literal),*) => {
        define_c_standard_header_folders!(@build_pattern [$($acc,)* $first, "|"] $($rest),*);
    };
}

define_c_standard_header_folders!(
    "sys",
    "arpa",
    "asm-generic"
);

fn main() {
    println!("{}", get_header_folders_pattern());
}
