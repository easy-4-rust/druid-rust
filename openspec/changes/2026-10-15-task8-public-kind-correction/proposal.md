# Correct Task 8 public kinds

## Why

The final JDBC semantic inventory cannot pass its public-API kind check even after implementation evidence is completed: five exception payload structs are labeled `ENUM_KIND`, `DriverRegistry` is labeled `MODULE`, the scanner misses the attributed `RowIdLifetime` enum, and the invalid-authorization row points to an associated constant rather than the actual enum variant. These are declaration mismatches, not adapter capability evidence.

## What Changes

- Correct the six inventory Rust shapes and the invalid-authorization canonical path to their actual public declarations.
- Recognize a `#[non_exhaustive]` prefix on a public enum without allowing associated constants to impersonate variants.
- Bind those exact accepted declaration migrations to the frozen Java oracle; keep all adapter, logical, capability, and runtime evidence states unchanged.
- Verify against a freshly generated RDBC public API and preserve the final gate's current PLANNED failure until implementation evidence is supplied.

## Scope

This change corrects declaration evidence only. It does not certify method coverage, live adapters, logical proxies, or production readiness.
