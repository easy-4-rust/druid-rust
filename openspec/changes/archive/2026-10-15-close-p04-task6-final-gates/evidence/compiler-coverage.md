# Task 6 compiler coverage identity regeneration

## Command

```powershell
$env:TEMP = 'E:\druid-rdbc-task6-isolated\tmp'
$env:TMP = 'E:\druid-rdbc-task6-isolated\tmp'
python scripts/regenerate_task6_coverage_manifest.py --write --timeout-seconds 3600
python scripts/render_task6_coverage_identity_map.py --write
python scripts/render_task6_coverage_identity_map.py
python -m unittest discover -s scripts -p 'test_regenerate_task6_coverage_manifest.py'
python -m unittest discover -s scripts -p 'test_render_task6_coverage_identity_map.py'
```

The local toolchain was `cargo-llvm-cov 0.9.1` with Rust 1.97.1 and `llvm-tools-preview`. Each of the seven families ran with its own temporary Cargo target, coverage JSON, and fresh `profraw`/`profdata` artifacts under E:. The generator checked unchanged build inputs before and after each family and wrote the manifest only after all runtime and compiler-identity checks passed.

## Result

```text
TASK6_COVERAGE_REGENERATION_FAMILIES=7
TASK6_COVERAGE_REGENERATION_IDENTITIES=105
TASK6_COVERAGE_MANIFEST_SHA256=7b1028399ddb0ccfb8f9fe975a24294714e6608014552b507b08581057eb61bd
TASK6_COVERAGE_REPORT_DUCKDB_SHA256=12646b2aaaf5b63a91910927d00b149607babeb86dfc66f62484f42aa7d46e3f
TASK6_COVERAGE_REPORT_HTTP_SQL_SHA256=29d0c38aa022543fe9ecf9ed92ed56b024a474db68defea19f6daf43bae5f4b5
TASK6_COVERAGE_REPORT_JDBC_AGENT_SHA256=eb6ac33fee91f6837c2ed1e79882bd03d3727e6ce40762ddd4397dbfaedc3cf0
TASK6_COVERAGE_REPORT_LIBSQL_SHA256=483da55485888d57403b458cb537eeb2386867109250bce479478ba8d788c889
TASK6_COVERAGE_REPORT_RBDC_SHA256=b9bc716e4d0d0935f512f1f0a1a8c724b6b1eb709f40ab422bb32f5a67e06f18
TASK6_COVERAGE_REPORT_SQLX_SHA256=728c89bdafd9bad53b47f4084ca8edf8529a779534f8261293ccf06c336707f3
TASK6_COVERAGE_REPORT_TOASTY_SHA256=0a850f284f57446b1fce54f88008e55caab208af7ae93278ea5e0543831f2266
TASK6_COMPILER_IDENTITY_MAP_ROWS=105
TASK6_COMPILER_IDENTITY_MAP_SHA256=fc42d1869bc413f709153e3f7a02fa41270f2e81feca95c28f38a4d61c8e8ff8
```

The regeneration tests passed 8/8 and the identity-map tests passed 3/3. The refreshed files are `docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json` and `docs/verification/rdbc-first/task6-compiler-identity-migration.tsv`.
