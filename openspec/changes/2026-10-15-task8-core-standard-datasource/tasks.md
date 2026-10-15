# Tasks

## 1. External contract

- [x] 1.1 Add an external-consumer test for standard acquisition, same-pool recycle, canonical unwrap identity, and unsupported credential SQLState; verify it fails before the missing core trait implementation.
- [x] 1.2 Implement the canonical `Wrapper` and `DataSource` traits directly on `DruidDataSource`; verify the external test and existing product data-source tests pass.

## 2. Integration and delivery

- [x] 2.1 Run formatter, strict Druid Clippy, relevant regression tests, and strict OpenSpec validation; record remaining broader CI or environment failures without promoting unrelated JDBC matrix rows.
- [x] 2.2 Commit and push this verified feature point; verify local and remote commit IDs match.
