import sys

filepath = 'crates/cpplint-core/src/runner.rs'
with open(filepath, 'r') as f:
    content = f.read()

search = """#[cfg_attr(feature = "hotpath", hotpath::measure)]
#[derive(Debug)]
pub struct Runner {"""

replace = """#[derive(Debug)]
pub struct Runner {"""

if search in content:
    content = content.replace(search, replace)
    with open(filepath, 'w') as f:
        f.write(content)
    print("Success")
else:
    print("Failed to find block")
