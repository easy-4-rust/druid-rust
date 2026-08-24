# P03 transitional standardization handoff

## Frozen recovery evidence

- Recovery HEAD: `c3772f40593da88d95ef699968ffd5c2fc59c132`.
- API command: `RUSTUP_TOOLCHAIN=nightly-2026-08-07-aarch64-apple-darwin cargo public-api --manifest-path crates/druid-rdbc/Cargo.toml -sss --all-features`.
- API snapshot: `api/druid-rdbc-spi.transitional.txt`, 2,317 lines, SHA-256 `5a75fc52f1a310859d5723e4f0f6358643ece8f34670b6afedcd676f4fd3aafc`.
- Physical/resource standardization inventory: `rdbc-physical-standardization.tsv`, 48 data rows, SHA-256 `a7cb3a105b764983e32101370d15cb49050b7177712bc6f76480a21bd827c28f`.
- Frozen Task 4 old/new public-path denominator: 285 data rows; SHA-256 `75c677e0ef6a0cb15122546f34e97f68b7687c46efe6feb53c94c1fe8c324d7a`.
- Current wrapper import denominator: 162 rows across 63 files and 10 families: 126 `RDBC_DIRECT` rows and 36 `DRUID_PRODUCT` rows. The before/after verifier passes.

The original generic/workspace-root `cargo public-api -p druid-rdbc -sss --all-features` invocation exited 1: the generic nightly was absent and cargo-public-api rejected the workspace virtual manifest. It produced an empty redirected file with SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`; it is not evidence. The fixed installed nightly plus crate-manifest command above is authoritative.

## Tasks 1–5 provenance

| Task | Commit range | Review state |
| --- | --- | --- |
| 1 | `f6a57bf..fac4e7b` | Complete; five review rounds closed with one explicitly parked source-only macro-scanner fidelity issue. |
| 2 | `2ff62be..7c72f83` | Complete; review clean. |
| 3 | `7c72f83..12755bb` | Complete; review clean after one fix round. |
| 4 | `12755bb..c590f01` | Complete; review clean after three fix rounds; 285-row map frozen. |
| 5 | `c590f01..401bf26` | Complete; review clean after two fix rounds; direct-wrapper import verifier passes. |

## Parked P03 verification issues

1. The source-only public-path scanner accepts a missing-semicolon parenthesis/bracket macro invocation. Cargo/rustc rejects it, so it is non-load-bearing but remains a verifier-fidelity follow-up.
2. The accepted Rust 1.95 MSRV build was not rerun; Task 2 used Rust 1.97.1.
3. The raw workspace all-target/all-feature test matrix has intermittent existing Druid singleton and Druid-admin process/port failures; isolated target runs pass and Task 5 did not change those components.

## Mandatory P04 entry condition

P03 status: TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION
P03 is not the accepted final ADR-RDBC-006 architecture.
P04 Task 1 must exit the DruidError waiver before standard trait unification.

`druid_rdbc::compat::DruidError` is a behavior-preserving transitional ownership seam, not the final error boundary. P04 Task 1 in [the single-standard plan](../../superpowers/plans/2026-08-24-rdbc-single-standard-unification.md) must replace it with the final typed error ownership before P04 Tasks 2–7 merge, internalize, move, or delete the rows in the frozen inventory. In particular, this handoff does not authorize a final standard API, a `Physical*` alias deletion, or a SemVer compatibility claim.

## Tooling boundary

`cargo semver-checks --version` is unavailable (`error: no such command: semver-checks`). It is `NOT_RUN`, not PASS; no tool was installed. Final SemVer validation remains P04 Task 8 work.
