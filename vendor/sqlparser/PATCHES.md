# Local patches to sqlparser 0.52.0

This directory vendors `sqlparser` 0.52.0 and is wired through
`[patch.crates-io]` in the workspace `Cargo.toml`.

## Why

Druid accepts Oracle `INSERT ... RETURNING <exprs> INTO <bind vars>`
(`DruidDataSource`-adjacent Wall/parser corpus). Upstream sqlparser 0.52 has
`Insert::returning` but no `INTO` bind list, so the statement failed to parse.

## Changes (keep this list in sync with the diff)

| File | Change |
|---|---|
| `src/ast/dml.rs` | `Insert` gains `returning_into: Option<Vec<String>>`; `Display` appends ` INTO a, b` |
| `src/parser/mod.rs` | after `RETURNING <items>`, GenericDialect accepts `INTO` followed by `:`-prefixed bind variables or placeholders |
| `src/ast/query.rs` + `src/ast/mod.rs` | `TableFactor::Table` gains `sample: Option<TableSample>` (`TABLESAMPLE (n PERCENT)`); `TableSample` exported |
| `src/parser/mod.rs` | parses Hive legacy `TABLESAMPLE (n [PERCENT])` after the table name |
| `src/test_utils.rs` | test constructors set `sample: None` |
| `src/ast/dml.rs` + `src/ast/mod.rs` | `CreateTable` gains `doris_unique_key`, `doris_distribution` (`DorisDistribution`), `doris_properties`; `Display` re-emits them |
| `src/ast/helpers/stmt_create_table.rs` | builder fields, setters and both constructors carry the three Doris fields |
| `src/parser/mod.rs` | GenericDialect parses `UNIQUE KEY (...)`, `DISTRIBUTED BY HASH (...) [BUCKETS n]`, `PROPERTIES (...)` |
| `src/keywords.rs` | adds `DISTRIBUTED` and `PROPERTIES` **in sorted position** (the tokenizer binary-searches this list) |

Bind variables are stored as raw strings (`":out_id"`) so formatting and
parameterization preserve the Oracle spelling.

## How to drop this patch

When sqlparser upstream supports `RETURNING ... INTO`, remove the
`[patch.crates-io]` entry, delete this directory, and re-run
`scripts/dialect_golden_diff.py` to confirm parity is unchanged.
