# Exact approved Druid differences — 2026-10-21

All code in this batch was completed before the unified benchmark module run. `cargo +1.97.1 test -p druid-bench --locked --features jdbc-agent --tests --no-fail-fast -- --include-ignored --nocapture` exited **0**, with **25 tests passed, zero failed/ignored**. This includes all nine live Java oracle targets and the exact-allowance per-field mutation checks.

Across independent scopes: **636 observations, 620 MATCH and 16 APPROVED_STANDARD_DIFFERENCE** (the previous two plus the fourteen now approved). The original frozen 59-case selection retains its source identities and now reports **47 MATCH, 12 approved differences, zero mismatches/unpaired cases**. This is Java Druid parity evidence, not full JDBC compliance. MC-08 and the full JDBC method denominator remain open.

The five comparators accept only exact scope, identity and both full observation records from the approved fixture. Any drift fails. Original failing comparisons remain in their historical folders. Current raw log: `D:/druid-rust-tooling/approved-java-differences-20261021.log`, SHA-256 `a34b338e1a2fcf40b0b4625d7b955f820fa7bd52e3de94392508c66cf3b59eb0`.

| Scope | Cases | Exact | Approved | Comparison SHA-256 |
| --- | --- | --- | --- | --- |
| callable | 7 | 2 | 5 | a0e893e032aebb213c205b01ddb89a07ae3ac89afaa545a8cf566dbdbfe61e56 |
| exception_sorters | 576 | 573 | 3 | 443a2cf1f04729623eefce819d7c07007c1278ea568c5099556ebc65ab925b35 |
| mock_sorter | 1 | 1 | 0 | eae005a3cbe441dc7fc3d935c043e00271cece09c68ac08a98e903e8ed0d4fa6 |
| pool | 6 | 3 | 3 | 35dc8040c577411e5a6c4b1186174984d0a1fb12af8e4537d8785b757121da2f |
| prepared_cache | 1 | 0 | 1 | 945a3f57bcff5f94c59adf4faee2ed421cdf5ef75052a9fceb305275ceaa3cee |
| result_set | 5 | 3 | 2 | f62c3b525a6ab55b02d8d13cfe87ed07b593166f9b5dd494d52d96de8b5f52da |
| statement | 37 | 35 | 2 | 66cb4244f8c3dc107f1cd54613ff4fa7c4f4b1d01bdf2732eb852912d39ae6ea |
| wall_multi | 2 | 2 | 0 | f6976faae74f13485dfaf5ee1f7563aeef79d12327c8442170aec36de92a5b6d |
| wall_table | 1 | 1 | 0 | 343d6e59786c9a0aef616aff72ac93af9cc6cb5c9c39c0fb1bd86e1344aba220 |
