# P04 Task 8: accept canonical file modules in purity gate

## Why

The final standard-purity verifier rejects `druid-rdbc/src/url.rs` because its physical-source denominator assumes every canonical root uses `module/mod.rs`. Rust permits a canonical root in `module.rs`, and the new standard URL parser uses that layout. The false failure blocks Task 8 while providing no additional purity guarantee.

## What Changes

- Accept a reachable canonical root stored in either `module.rs` or `module/mod.rs`.
- Keep every physical child Rust file within a file module's directory in the pollution denominator.
- Reject an ambiguous root containing both forms, and retain the existing directory-module minimum source check.
- Add self-tests for accepted file roots and rejected polluted or ambiguous roots.

## Scope

This changes only the verifier and its tests. It does not weaken forbidden identity/path checks or claim Task 8 completion.
