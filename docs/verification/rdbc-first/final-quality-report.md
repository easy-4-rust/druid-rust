# RDBC-first quality and public API gate

Source: `078dfa2` (last product/benchmark source change `ead29ff`), 2026-10-18. The [driver matrix run 39112028023](https://github.com/easy-4-rust/druid-rust/actions/runs/39112028023) completed **44/44 jobs successfully**. A job conclusion of `success` means its listed commands exited 0; it does not by itself establish broader semantic or performance acceptance.

| Check | Evidence | Verdict |
| --- | --- | --- |
| Format, default and all-feature checks, strict Clippy, rustdoc | `rust-contracts` on Ubuntu completed successfully with `cargo fmt --all -- --check`, workspace all-target default and all-feature checks, `cargo clippy --workspace --all-targets --all-features -- -D warnings -A clippy::pedantic`, and `cargo doc --workspace --all-features --no-deps`. | PASS on this runner |
| Default workspace tests | Same job: 324 suite result lines, 4,276 passed, 0 failed, 3 ignored. | PASS on this runner |
| All-feature workspace tests | Same job: 334 suite result lines, 4,458 passed, 0 failed, 6 ignored. | PASS on this runner |
| Optional-feature isolation | Same job passed the isolated-driver, no-default Wrapper/benchmark, Toasty SQLite runtime, and catalog-only runtime steps. These checks are the repository's explicit feature matrix; the plan's literal `cargo hack --feature-powerset` command has not been shown to run. | PASS for explicit matrix; literal powerset NOT_RUN |
| MSRV | `msrv` job passed `cargo check --workspace --all-targets` on the configured Rust 1.95 toolchain. | PASS for configured command |
| Public API snapshots | `public-api` job passed snapshot verification and uploaded observed API artifacts. The checked-in snapshots are the authoritative baseline; locally untracked `api/observed/` is preexisting and remains untouched. | PASS for snapshot check |
| SemVer migration | `semver` job passed report capture and the exact approved pre-release migration verifier for `druid-rdbc`, `druid`, and `druid-wrapper`. This permits only documented differences from the frozen baseline. | PASS for approved pre-release policy |
| External `druid::sql::*` consumer | `task6_rdbc_external_consumer_test_parity.rs` compiles an independent consumer crate through the legacy path and checks type identity. The default workspace job log records 3 passed, 0 failed, including the external consumer public-contract test. | PASS on this runner |

The macOS x86_64 SQLite jobs and SQLite aggregate-evidence job also succeeded. The overall P07 release verdict stays `NOT_READY` for the explicitly identified semantic and microbenchmark-comparison gaps in the [release report](final-release-report.md).
