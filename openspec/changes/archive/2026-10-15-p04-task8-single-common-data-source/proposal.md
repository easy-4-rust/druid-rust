# P04 Task 8: one CommonDataSource contract

## Why

The final no-duplicate-standard gate identifies `druid::sql::CommonDataSource` as a second public trait with the same JDBC surface as `druid-rdbc::data_source::CommonDataSource`. The older trait returns integer seconds and hides read errors, while the accepted standard contract uses `Duration` and a fallible getter. Data sources currently implement both.

## What Changes

- Make `druid::sql::CommonDataSource` a named re-export of the canonical RDBC trait, keeping the import path while removing the second trait identity.
- Move affected Druid and wrapper data-source implementations and tests to the canonical signatures and error semantics.
- Record the pre-release source/API migration; run focused data-source tests, public API checks, and the final no-duplicate-standard verifier.

## Scope

This closes the duplicate `CommonDataSource` finding only. The Druid-specific `DataSource` connection-return type and other Task 8 gates remain separate work.
