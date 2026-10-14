# Task 6 deleted-test parity execution

## Command

```powershell
python scripts/verify_task6_deleted_test_parity.py
```

The command completed successfully against the current checkout with locked, offline Cargo dependencies and Rust 1.97.1.

## Result

```text
PASS task6 deleted-test parity: files=67 base_tests=1283 migrated_exact=1146 parameterized=137 pure_identity=0 approved_removal=0 zero_test_identity=1 zero_test_behavioral_handoff=1 executed=yes
```

The checked ledger is `docs/verification/rdbc-first/task6-deleted-test-migration.tsv` with SHA-256 `EBE9B41694E8FE572BD428D1650BC7999B80B005EE903615A135329A7B2DA8AE`.

This establishes that all 1,283 baseline test identities are accounted for, behavioral targets are executable, the exact/parameterized observation floors pass, and the verifier executed the target tests rather than only checking structure.
