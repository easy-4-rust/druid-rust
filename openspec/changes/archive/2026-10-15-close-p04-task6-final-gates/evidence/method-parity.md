# Task 6 method parity execution

## Command

```powershell
python scripts/verify_task6_method_parity.py
```

The command completed successfully against the current checkout using the repository's locked, offline Cargo dependencies and the `stable` host toolchain (Rust 1.97.1).

## Result

```text
TASK6_METHOD_ROWS=479
TASK6_METHOD_OBJECTS=ConnectionExt=19,PhysicalCallableStatement=32,PhysicalConnection=52,PhysicalDatabaseMetaData=173,PhysicalPreparedStatement=32,PhysicalResultSet=120,PhysicalResultSetMetaData=21,PhysicalStatement=30
TASK6_METHOD_DISPOSITIONS=INTERNALIZED=2,MERGED_EXACT=212,MERGED_TYPED=265
TASK6_METHOD_SHA256=0b1e1357ae9b416264696be42a916e68cef1b8d5cd23fb6cb4c68933e55877e5
TASK6_METHOD_EVIDENCE_ROWS=479
TASK6_METHOD_EVIDENCE_UNIQUE=442
TASK6_METHOD_EVIDENCE_C2_ROWS=43
TASK6_METHOD_EVIDENCE_FALLBACK=436
TASK6_METHOD_EVIDENCE_PARAMETERIZED_ROWS=33
TASK6_METHOD_EVIDENCE_SHA256=06584b96cda7dbf0a07affe4ecdddbc144252610bd2eee96b0d39474eb205d96
TASK6_METHOD_EVIDENCE_TARGETS=7
TASK6_METHOD_EVIDENCE_FUNCTIONS=442
TASK6_METHOD_EVIDENCE_SELECTORS=442
TASK6_METHOD_EVIDENCE_EXECUTED=yes
```

The structural phase also completed its C2 source check against the 1,283-row deleted-test ledger. This does not substitute for running all deleted-test parity targets; that remains Task 2.2.
