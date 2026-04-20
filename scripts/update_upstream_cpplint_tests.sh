#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/tests/upstream_cpplint"

mkdir -p "${TARGET_DIR}"

curl -L --fail \
  https://github.com/cpplint/cpplint/raw/develop/cpplint.py \
  -o "${TARGET_DIR}/cpplint.py"

curl -L --fail \
  https://github.com/cpplint/cpplint/raw/develop/cpplint_unittest.py \
  -o "${TARGET_DIR}/cpplint_unittest.py"

echo "Updated upstream cpplint test files in ${TARGET_DIR}"
