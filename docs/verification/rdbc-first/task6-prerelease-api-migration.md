# Task 6 pre-release API migration record

## Decision and scope

The `0.0.0-design` crates are pre-release. The public API migration removes duplicate pooled-object and filter-chain surfaces rather than retaining parallel APIs. The supported application entry remains `druid::DataSource`, followed by the canonical `druid-rdbc` `Connection`, `Statement`, `PreparedStatement`, `CallableStatement`, `ResultSet`, and metadata contracts. This is an intentional pre-release breaking change; downstream users must migrate before a stable release.

## SemVer inventory

The SemVer report contains 2,968 removed-method occurrences in `druid`: 2,578 `inherent_method_missing` occurrences and 390 `trait_method_missing` occurrences. SemVer repeats identities reached through duplicate public paths. The checked-in ledger therefore has 831 unique `(lint, symbol, baseline source, line)` identities and stores an occurrence count on each row; the counts sum back to 2,968.

| Finding group | Report occurrences | Unique source identities | Route |
|---|---:|---:|---|
| Inherent methods | 2,578 | 636 | 410 identities point to the standard connection, statement, prepared/callable statement, result-set, or metadata contract family; 226 identify retired pool/filter implementation helpers. |
| Trait methods | 390 | 195 | Retired `ResultSetFilter`, `BeforeFilter`, and `StatFilterContextListener` filter/statistics extension surfaces. |
| **Total** | **2,968** | **831** | Every unique identity is present in `api/task6-prerelease-method-migration.tsv`. |

The item ledger records the exact old identity, baseline file and line, repeated finding count, disposition, and route. The existing category allowlist continues to pin each crate/lint count and fingerprint. The SemVer verifier now checks the item ledger against the live report, so a new or missing identity, source change, or count drift fails CI. The 479-row behavioral method-migration ledger and 1,283-row deleted-test ledger answer a different question: which behavior was preserved or intentionally removed. The item ledger does not claim one-to-one replacement for every convenience method.

## Verification record

- Baseline revision: `2c421efa6caf8bfc983633529fc3775c251f9492`.
- Completed SemVer report artifact: [run 38661619894](https://github.com/easy-4-rust/druid-rust/actions/runs/38661619894), generated from commit `115f3ce`.
- The reports from that run yielded 27 approved breaking categories across `druid-rdbc`, `druid`, and `druid-wrapper` (24 major and 3 minor checks); the exact report findings are category-fingerprinted in `api/rdbc-single-standard-breaking.allowlist.tsv`.
- Current SemVer verification passed on commit `6f2b993` in [run 38664321479](https://github.com/easy-4-rust/druid-rust/actions/runs/38664321479) (SemVer job [116193031716](https://github.com/easy-4-rust/druid-rust/actions/runs/38664321479/job/116193031716)). The extracted artifact was replayed locally through the new item-level verifier and yielded `Verified 27 exact approved SemVer breaking categories`; focused verifier tests passed 8/8.
- SHA-256: SemVer reports — `druid-rdbc` `B953670271D3DA714887DBE1A5A8B57C577333C44B89472F8F7B9E200766CC70`, `druid` `8876509A4DC1186F6C6C051BBD8F7516528D5A32C3251D08887969A560BC89C3`, `druid-wrapper` `3F9838A9BAB9AE3D88BFD2EFEE73E0F58F8A348348C206DCC6BC8B1E552D25B9`; method ledger `0067F77298A318EBD9759CF39B7E68035A070A90ED09EA959FA37329A222A870`; category allowlist `1F77430520F19B94A7CC5203A7D80A7B7BC54C8C4B9A6C64C09875336FA544AF`.
- The entire run `38664321479` is still checking cross-platform and real-database contracts; this record claims only the successful SemVer job. The migration change's own code and verifier changes have been committed separately as `06f69c9` and will receive a new CI run after push.

This migration record closes only the method-removal accounting within Task 6. It does not close the broader P04 Task 6/Task 8 acceptance gates or establish production readiness.
