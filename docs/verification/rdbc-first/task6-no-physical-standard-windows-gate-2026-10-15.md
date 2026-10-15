# Task 6 physical-standard gate on Windows (2026-10-15)

The exact final gate completed with exit code 0 on `main` after the child
method-parity budget was raised from 600 to 1,800 seconds. Its runtime evidence
was executed; `--no-execute` was not used for the final result.

The Windows run placed temporary files and the Cargo target directory on D:
and set `CARGO_PROFILE_TEST_DEBUG=0` to avoid an MSVC PDB `LIMIT (12)` linker
error on the nearly full E: drive. These settings change only build artifact
location and debug symbols, not test behavior or assertions.

```text
PRODUCTION_RUST_FILES_SCANNED=815
PUBLIC_PHYSICAL_NAMES=0
PRODUCTION_PHYSICAL_NAMES=0
RUSTDOC_PHYSICAL_NAMES=0
DRUID_RDBC_SPI_MODULES=0
MIGRATION_IDENTITIES_CLOSED=79
NEWLY_DISCOVERED_IDENTITIES_CLOSED=11
TRANSITIONAL_CONSUMER_IDENTITIES_CLOSED=75
DELETED_TEST_FUNCTION_MIGRATIONS=1283
PUBLIC_CONFIG_WIRE_BREAKS_DISPOSITIONED=128
ACTIVE_TRANSITIONAL_CONSUMERS=0
```

Command: `python scripts/verify_no_physical_standard.py` with `TEMP` and `TMP`
set to `D:\druid-rust-task8-temp`, `CARGO_TARGET_DIR` set to
`D:\druid-rust-task8-target`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`PYTHONUTF8=1`. This receipt proves this gate only; the P04 Task 8 final semantic,
SemVer, real-database, and full workspace gates remain separate.
