# RDBC error waiver exit

- Baseline: `bb91193d9bb488b107b8c9ac6dd56f8756ed9593`
- Disposition: `docs/verification/rdbc-first/rdbc-error-disposition.tsv`
- Live rows: `90`
- Manifest SHA-256: `7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603`
- Domain rows: `RDBC_COMPAT=2`, `RDBC_STANDARD=37`, `WRAPPER_ADAPTER=51`

## Current gate run (2026-10-11, `017f571`)

Command: `python scripts/verify_rdbc_error_waiver_exit.py --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv`.

- `WAIVER_EXIT_GATE_1_COMPAT_REMOVED`: PASS — transitional definition/export absent
- `WAIVER_EXIT_GATE_2_STANDARD_CLEAN`: PASS — standard tree uses RdbcError
- `WAIVER_EXIT_GATE_3_ADAPTERS_CLEAN`: PASS — 61 adapter file(s) use RdbcError
- `WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE`: PASS — Druid uniquely owns DruidError and wraps RdbcError
- `WAIVER_EXIT_GATE_5_BASELINE_DISPOSED`: FAIL — 30 baseline paths were removed during standardization and 3 surviving paths contain no `RdbcError`; the frozen path-based disposition has not been reconciled with their canonical replacements.
- `WAIVER_EXIT_GATE_6_COMPAT_VERIFIED`: FAIL — prerequisite gate 5 is not green, so focused verification is not credited.

The baseline denominator and manifest hash above remain unchanged. This report records the live gate result; removed paths require reviewed replacement evidence before waiver exit can be claimed.
