# Remove vulnerable ZIP build dependency

## Why

The release CI supply-chain job fails on three RustSec advisories against `zip 6.0.0`. That crate enters through `libduckdb-sys 1.10505.0`, whose build script calls `ZipArchive::extract`. A production-ready release cannot treat this as a green gate.

## What Changes

Upgrade the pinned DuckDB Rust binding to an upstream release whose `libduckdb-sys` build dependency uses patched `zip >=9.0.0`. Refresh the lockfile and preserve the DuckDB adapter's supported behavior. Do not waive these advisories.

## Impact

Workspace dependency pin, `Cargo.lock`, DuckDB feature/build matrix, release security checks. No public RDBC API change is intended.
