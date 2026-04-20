# Upstream cpplint.py Tests

This directory vendors upstream `cpplint.py` and `cpplint_unittest.py` from:

- <https://github.com/cpplint/cpplint>
- branch: `develop`

## Files

- `cpplint.py`
- `cpplint_unittest.py`

## Run

From repository root:

```bash
just test-upstream-cpplint
```

Or directly:

```bash
cd tests/upstream_cpplint
python3 -m pytest cpplint_unittest.py
```

## Notes

- `pytest` is required (`python3 -m pip install pytest`).
- The tests are kept upstream-compatible and should not be edited unless necessary.
