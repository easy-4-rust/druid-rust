# Task 6 compiler coverage identity regeneration

## Command

```powershell
$env:TEMP = 'D:\codex_tmp\druid_task6_coverage'
$env:TMP = 'D:\codex_tmp\druid_task6_coverage'
python scripts/regenerate_task6_coverage_manifest.py --write --timeout-seconds 3600
python scripts/render_task6_coverage_identity_map.py --write
python scripts/render_task6_coverage_identity_map.py
python -m unittest discover -s scripts -p 'test_regenerate_task6_coverage_manifest.py'
python -m unittest discover -s scripts -p 'test_render_task6_coverage_identity_map.py'
```

The current approved local toolchain was `cargo-llvm-cov 0.9.1` with Rust 1.97.1 and `llvm-tools-preview`. Each of the seven families ran with its own temporary Cargo target, coverage JSON, and fresh `profraw`/`profdata` artifacts under D:. The generator checked unchanged build inputs before and after each family and wrote the manifest only after all runtime and compiler-identity checks passed.

## Result

```text
TASK6_COVERAGE_REGENERATION_FAMILIES=7
TASK6_COVERAGE_REGENERATION_IDENTITIES=105
TASK6_COVERAGE_MANIFEST_SHA256=9ffee743452b1d2718c719bbef6e0ee61d22d42bd8d8c99b255aeb1c6e2d6e62
TASK6_COVERAGE_REPORT_DUCKDB_SHA256=88f373215a0364ffaff58a2a335acd1a21629b228f18e0755f9977e9b78855ed
TASK6_COVERAGE_REPORT_HTTP_SQL_SHA256=cac9989e41a273227b78b55226de5cf7cefa29bd6e02041ef6c27a8d1fc1b865
TASK6_COVERAGE_REPORT_JDBC_AGENT_SHA256=fed2e29c23c61c1a7326f129d3d8d5e6f1fa6294da264ccbf7f6b90ea7f68126
TASK6_COVERAGE_REPORT_LIBSQL_SHA256=775fc6c7d3b4aa7a57e627f21b8f6bed419aecd02856f26fc3b10ab6f7f76ccb
TASK6_COVERAGE_REPORT_RBDC_SHA256=5c080b3c485614976f98668919f6496ffb2f02af2bacca124985d9df00a4cfd1
TASK6_COVERAGE_REPORT_SQLX_SHA256=7d260926b23f3cfd571486dc22eb815795a33f9558e803a46cad2d05dc72a0a9
TASK6_COVERAGE_REPORT_TOASTY_SHA256=beab7ad64465cffafaff27fe11121a45a7cba1eb33905b5dc2d37dd3ac0661a2
TASK6_COMPILER_IDENTITY_MAP_ROWS=105
TASK6_COMPILER_IDENTITY_MAP_SHA256=35a150b8ae8eed776a3fdacccfbb850e9356da280af962b30e93a085cd7372d3
```

The regeneration tests passed 8/8 and the identity-map tests passed 3/3. The refreshed files are `docs/verification/rdbc-first/rdbc-adapter-coverage-regions.json` and `docs/verification/rdbc-first/task6-compiler-identity-migration.tsv`.
