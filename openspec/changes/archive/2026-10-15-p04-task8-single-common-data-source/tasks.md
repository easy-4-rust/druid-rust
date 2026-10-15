# Tasks

- [x] 1. Update focused tests to expose the old trait's incompatible default and ensure the canonical import path/semantics are asserted.
- [x] 2. Re-export the canonical trait, migrate product and wrapper implementations, and pass focused tests plus the no-duplicate-standard gate. Make that gate invoke its Python child through the current interpreter on Windows.
- [x] 3. Review public API/SemVer migration evidence, validate OpenSpec and relevant regression tests, record limits, then commit and push this feature point.

## Verification record

- `cargo test -p druid --test common_data_source_trait_test`: 13 passed.
- `cargo test -p druid --test common_data_source_logging_pool_test`: 4 passed.
- `cargo test -p druid-wrapper --test common_data_source_public_examples_test`: 1 passed.
- `cargo test -p druid-wrapper --test common_data_source_logging_test`: 2 passed.
- `python -m unittest scripts.test_verify_no_physical_standard`: 9 passed (with `PYTHONPATH=scripts`).
- Direct `verify_no_duplicate_trait_tree` invocation: passed; public contract gate: 128 allowlist rows, passed; migration authority gate: 79 rows, passed.
- `rustfmt --check` for changed Rust files and `openspec validate --strict`: passed.
- Full `verify_no_physical_standard.py` remains unverified on this Windows host: the method-parity child exceeded its existing 600-second timeout after the Python interpreter portability fix. This does not close all P04 Task 8 gates or claim production readiness.
