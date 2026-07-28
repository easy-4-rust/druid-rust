<a id="readme-top"></a>

<div align="center">

# druid-rust

**A planned semantic migration of Alibaba Druid (Java) to Rust**

[English](./README.md) | [简体中文](./README.zh-CN.md)

[Overview](#1-project-positioning-and-status) · [Features](#2-features-and-maturity) ·
[Architecture](#4-workspace-and-crate-architecture) ·
[Design sketch](#6-design-sketch-future-api-not-runnable-today) ·
[Roadmap](#11-roadmap-and-phases) ·
[Contributing](#19-contributing-security-and-license)

</div>

---

> **Project status: Active semantic migration.**
>
> This repository is a buildable Cargo workspace with implemented core, pool,
> SQL, statistics, dynamic datasource and driver-bridge code. It is not yet a
> stable or complete Druid migration: every completed claim must be backed by
> Java-oracle differential tests or an explicit Rust ecosystem contract.
>
> The goal is complete Druid feature semantics, delivered in planned slices.
> It is deliberately not a line-for-line Java translation, but it is also not
> a loose reimplementation that may drop behavior.

> **Last verified**: 2026-07-28.

## 1. Project Positioning and Status

**druid-rust is a Rust workspace that migrates Alibaba Druid's observable
semantics to the Rust async ecosystem.** Connection pooling, filter chains,
SQL firewall, statistics, dynamic datasource and the admin surface are tracked
as migration contracts against the Java source rather than selected only as
general architectural patterns. The implementation targets
(`tokio`, `sqlparser-rs`, `sqlx`, `deadpool`, `bb8`, `rbdc`, `axum`).

### 1.1 What this project is

| Field | Value |
| :--- | :--- |
| Primary contracts | `druid-core`; native pool in `druid-pool` |
| Current version | `0.0.0-design` (unstable migration workspace) |
| MSRV | `1.75` (pinned in `rust-toolchain.toml`) |
| Edition | `2021` |
| Workspace resolver | `2` |
| Unsafe policy | `forbid` (workspace lint) |
| Default features | none |
| Publication | none — every crate is `publish = false` |
| License | Apache-2.0 |

### 1.2 What this project is not

- Not a line-for-line or class-layout copy. Java behaviors are migrated through
  an object ledger and semantic contract matrix, with Rust-specific internal
  scheduling and driver adapters where the platforms differ.
- Not an ORM. druid-rust does not generate SQL; it pools, observes and
  governs SQL emitted by the host application.
- Not a database migration tool. Schema versioning is explicitly out of scope.
- Not release-ready. The workspace is runnable and tested, but the complete
  Druid object/behavior matrix, Java golden tests and stable API are unfinished.

### 1.3 Status evidence

| Claim | Current value | Evidence |
| :--- | :--- | :--- |
| Workspace builds | yes with current stable toolchain | `cargo +stable test --workspace --all-targets` |
| Connection API | implemented, unstable | `DruidPooledConnection` → `PhysicalConnection` |
| Tests | implemented | core/pool/SQL/Stats/Dynamic/SQLx/RBDC/bb8/deadpool suites |
| Documentation | migration ledger + architecture | `doc/migration/` |
| crates.io | not published | every crate is `publish = false` |
| docs.rs | not published | every crate is `publish = false` |
| CI | not configured | no `.github/workflows/` directory |
| Coverage | baseline measured; current changes require re-audit | `cargo llvm-cov` is an exit gate, not a completion shortcut |
| Benchmark | not measured | no `benches/` directory |

## 2. Features and Maturity

### 2.1 Feature matrix

| Feature | Status | Crate | Constraint | Verification |
| :---: | :---: | :--- | :--- | :--- |
| Driver-agnostic `PhysicalConnection` SPI | 🚧 partial | `druid-core` | JDBC breadth incomplete | migration doc §5 |
| Native async pool | 🚧 partial | `druid-pool` | full Druid lifecycle incomplete | migration semantic matrix |
| sqlparser-rs based Wall rules | 🚧 partial | `druid-sql` | Druid rule matrix incomplete | migration semantic matrix |
| SQL merge statistics | 🚧 partial | `druid-stats` | layered Java stats incomplete | migration semantic matrix |
| Multi-datasource hot switching | 🚧 partial | `druid-dynamic` | HA health/recovery incomplete | migration semantic matrix |
| `sqlx + deadpool` external-pool bridge | 🧪 preview | `druid-sqlx-deadpool` | no nested Druid pool | bridge contract tests |
| `sqlx + bb8` external-pool bridge | 🧪 preview | `druid-sqlx-bb8` | no nested Druid pool | bridge contract tests |
| SQLx direct adapter | 🧪 preview | `druid-sqlx` | SQLite tested; DB matrix incomplete | direct contract tests |
| RBDC direct adapter | 🚧 partial | `druid-rbdc` | real driver matrix incomplete | RBDC trait contract tests |
| `/druid/admin` HTTP surface | 🗓️ design target | `druid-admin` | V3 | `doc/9` |
| SQL injection regex detector | ⛔ not ported | — | semantically unsafe | ADR-005 |

### 2.2 Status legend

| Status | Definition |
| :--- | :--- |
| ✅ stable | Public API, tests, docs and compatibility commitments are complete |
| 🧪 preview | Usable but API or behaviour may change |
| 🚧 partial | Only the explicitly listed subset works |
| 🗓️ planned | No callable implementation exists yet |
| ⛔ not ported | Deliberately rejected — see ADR for rationale |

## 3. Rust Baseline and Platform Support

| Item | Value | Source |
| :--- | :--- | :--- |
| MSRV | `1.75` | `rust-toolchain.toml`, `Cargo.toml` `rust-version` |
| Edition | `2021` | workspace `[workspace.package]` |
| Resolver | `2` | workspace `[workspace]` |
| rustfmt | stable | `rust-toolchain.toml` components |
| Clippy | workspace pedantic enabled | `Cargo.toml` `[workspace.lints.clippy]` |

> Platform support, `no_std` and WASM remain undecided. `druid-core` already
> exposes public contracts, but no stable platform matrix is claimed yet.

## 4. Workspace and Crate Architecture

### 4.1 One-screen view

```text
[downstream application]
        │
        ▼
┌──────────────────────────────────────────────────────────────┐
│ druid-rust Cargo Workspace (active migration)                │
│                                                              │
│ druid-core         DruidPooledConnection / PhysicalConnection│
│ druid-sql          sqlparser-rs AST, Wall, parameterize      │
│ druid-pool         Druid native async pool                    │
│ druid-stats        SQL merge + percentile + Prometheus        │
│ druid-dynamic      ArcSwap multi-datasource + read/write split
│                                                              │
│ druid-sqlx         direct SQLx PhysicalConnection adapter     │
│ druid-rbdc         direct RBDC PhysicalConnection adapter     │
│ druid-sqlx-deadpool external pool bridge                      │
│ druid-sqlx-bb8     external pool bridge                       │
│                                                              │
│ druid-admin        axum-based /druid/admin HTTP surface       │
└──────────────────────────────────────────────────────────────┘
        │
        ▼
[database driver ecosystem: sqlx, rbdc, deadpool, bb8, tokio-postgres, ...]
```

The connection boundary is fixed:

```text
DruidPooledConnection
└── PhysicalConnection
    ├── SqlxConnectionAdapter
    ├── RbdcConnectionAdapter
    └── other driver adapters
```

bb8/deadpool are `Pool` providers. They return an external lease through
`PhysicalConnectionLease`; they are not `PhysicalConnectionFactory`
implementations and are never nested inside `DruidPool`.

### 4.2 Dependency graph

```mermaid
flowchart TB
    APP["downstream application"] --> CORE["druid-core"]
    APP --> POOL["druid-pool"]
    APP --> DYNAMIC["druid-dynamic"]
    APP --> ADMIN["druid-admin"]

    SQL["druid-sql"] --> CORE
    POOL --> CORE
    STATS["druid-stats"] --> CORE
    STATS --> SQL
    DYNAMIC --> CORE
    DYNAMIC --> SQL
    DYNAMIC --> POOL

    RBDC["druid-rbdc"] --> CORE
    SQLX["druid-sqlx"] --> CORE
    SQLXDP["druid-sqlx-deadpool"] --> CORE
    SQLXDP --> SQLX
    SQLXB8["druid-sqlx-bb8"] --> CORE
    SQLXB8 --> SQLX

    ADMIN --> CORE
    ADMIN --> POOL
    ADMIN --> STATS
    ADMIN --> DYNAMIC
```

### 4.3 Crate map

| Crate | Publish | Default | Responsibility | Planned key dependencies |
| :---: | :---: | :---: | :--- | :--- |
| `druid-core` | ⛔ | — | `DruidPooledConnection`, `PhysicalConnection`, `Pool`, Filter contracts | `async-trait` |
| `druid-sql` | ⛔ | — | sqlparser-rs adapter, Wall rules, parameterization, fingerprint | `sqlparser` |
| `druid-pool` | ⛔ | — | HikariCP-style async pool, idle queue, eviction scheduler | `tokio`, `parking_lot` |
| `druid-stats` | ⛔ | — | SQL merge, percentile histogram, Prometheus exporter | `moka`, `prometheus` |
| `druid-dynamic` | ⛔ | — | `ArcSwap` multi-datasource, read/write split, load balancer | `arc-swap`, `dashmap` |
| `druid-sqlx` | ⛔ | — | Direct raw SQLx connection adapter | `sqlx` |
| `druid-rbdc` | ⛔ | — | Direct adapter for the `rbdc` driver ecosystem | `rbdc`, `rbs` |
| `druid-sqlx-deadpool` | ⛔ | — | deadpool external-pool bridge over SQLx | `druid-sqlx`, `deadpool` |
| `druid-sqlx-bb8` | ⛔ | — | bb8 external-pool bridge over SQLx | `druid-sqlx`, `bb8` |
| `druid-admin` | ⛔ | — | axum-based `/druid/admin` HTTP surface | `axum`, `prometheus` |

### 4.4 Dependency and visibility rules

- `druid-core` MUST NOT depend on any driver, parser, async runtime or TLS
  backend. It only exposes trait contracts.
- Domain crates (`druid-sql`, `druid-pool`, `druid-stats`, `druid-dynamic`)
  MUST NOT depend on each other cyclically; they share `druid-core` only.
- Direct adapters depend on `druid-core`; external-pool bridges depend on
  `druid-core`, `druid-sqlx` and exactly one pool ecosystem. They MUST NOT
  depend on `druid-pool`, because that would create pool-in-pool.
- `druid-admin` is the only crate that depends on every other crate; it is
  intentionally not in the dependency closure of any domain crate.
- All crates are `publish = false` until Phase 1 closes.

## 5. Design Principles

| Principle | How it lands in this workspace | Verified by |
| :--- | :--- | :--- |
| Type safety | `PhysicalConnection` is internal behind `Box<dyn PhysicalConnection>`; applications receive `DruidPooledConnection` | compile + contract tests |
| Clear ownership | `DruidPooledConnection` owns one connection and one `FnOnce` return path; external leases use `PhysicalConnectionLease` | exactly-once tests |
| Composable errors | Single `druid_core::Error` enum with `thiserror`; `Result<T, Error>` propagates through all layers | future error tests |
| Default safe | Wall denies `DROP`/`TRUNCATE` by default; filter chain defaults to `StatFilter + WallFilter` | future security tests |
| Zero-cost where it matters | Trait dispatch through `async-trait` only at the boundary; inner scheduler uses `parking_lot::Mutex` and `tokio::sync::Notify` | future bench |
| Evolvable | Feature flags hide individual adapters; the `druid-core` trait surface is the contract that survives upgrades | future semver tests |

## 6. Historical Design Sketch

> **Warning: the snippets below are preserved historical design sketches.** Crate names, module
> paths, trait signatures and configuration keys are *provisional* and will
> change before any crate is published. Do not copy them into production
> code. They exist to communicate intent, not to serve as an API guarantee.

### 6.1 Minimum example (planned)

```rust
// pseudocode — does NOT compile against the current workspace
use druid_core::pool::DruidPool;
use druid_sqlx_bb8::SqlxBb8PoolBuilder;

let pool: DruidPool<_> = SqlxBb8PoolBuilder::new()
    .driver_name("postgres")
    .url("postgres://user:pwd@host/db")
    .max_open(20)
    .max_idle(4)
    .acquire_timeout(Duration::from_secs(3))
    .build()
    .await?;

let mut conn = pool.get().await?;
let rows = conn.fetch("SELECT id FROM users WHERE id = ?", vec![42.into()]).await?;
println!("rows = {rows:?}");
```

### 6.2 Multi-datasource hot switch (planned)

```rust
// pseudocode — does NOT compile against the current workspace
use druid_dynamic::{DynamicDataSource, SqlHint};

let ds = DynamicDataSource::builder()
    .add("main", main_pool)
    .add("readonly", read_pool.clone())
    .build();

// later, with zero downtime, swap the master:
ds.switch("new_main").await?;

// route based on the statement kind
let conn = ds.route(SqlHint::Read).await?;
```

### 6.3 Wall rule (planned)

```rust
// pseudocode — does NOT compile against the current workspace
use druid_sql::wall::WallConfig;

let wall = WallConfig::default()
    .deny_drop_table(true)
    .deny_truncate(true)
    .update_must_have_where(true)
    .delete_must_have_where(true)
    .max_sql_length(Some(64 * 1024))
    .build();
```

## 7. Planned Cargo Features

> **Note: every crate currently has zero features.** This section is a
> design sketch and will be populated once `druid-core` exposes its trait
> surface in V1.

| Crate | Feature | Default | What it enables | Planned key dependency |
| :---: | :---: | :--- | :--- | :--- |
| `druid-core` | `std` | yes | use `std::error::Error` and `std::time::Duration` | none |
| `druid-sql` | `postgres-dialect` | no | `PostgreSqlDialect` parameterization | `sqlparser` |
| `druid-sql` | `mysql-dialect` | no | `MySqlDialect` parameterization | `sqlparser` |
| `druid-sqlx-deadpool` | `postgres` | no | `sqlx::postgres` feature gate | `sqlx` |
| `druid-sqlx-deadpool` | `mysql` | no | `sqlx::mysql` feature gate | `sqlx` |
| `druid-admin` | `tls` | no | rustls server config | `axum`, `rustls` |

## 8. Planned Public API and Usage

The trait surface planned for V1 is summarized below; see
`druid-rust-Architecture.zh_CN.md` §8 and `doc/7、druid-rust-领域模型设计.md`
for the full contract.

```rust
// planned — not implemented
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, Error>;
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, Error>;
    async fn begin(&mut self) -> Result<(), Error>;
    async fn commit(&mut self) -> Result<(), Error>;
    async fn rollback(&mut self) -> Result<(), Error>;
    async fn ping(&mut self) -> Result<(), Error>;
    async fn close(&mut self) -> Result<(), Error>;
}
```

## 9. Backends, Formats and Optional Engines

| Capability | Backend | Planned feature | Boundary | License |
| :--- | :--- | :--- | :--- | :--- |
| Postgres | `tokio-postgres` via `sqlx` | `druid-sqlx-deadpool/postgres` or `druid-sqlx-bb8/postgres` | parameter-binding required | Apache-2.0 / MIT |
| MySQL | `sqlx::mysql` | `druid-sqlx-deadpool/mysql` | placeholder rewriting via `druid-sql` | Apache-2.0 / MIT |
| MSSQL | `tiberius` (planned) | `druid-sqlx-bb8/mssql` (planned) | not in V1/V2 scope | Apache-2.0 |
| rbdc ecosystem | `rbdc-pg`, `rbdc-mysql`, `rbdc-mssql` | `druid-rbdc/<dialect>` | bridges through `Box<dyn Connection>` | Apache-2.0 |
| SQLite / DuckDB / Turso | n/a | n/a | explicitly out of scope | — |

> Multi-engine parity is not guaranteed. If a backend lacks a feature the
> `Pool::state()` and the `druid-admin` API will report `Unsupported` rather
> than silently degrade.

## 10. Concurrency, Memory and Resource Model

- `Send + Sync`: every public type must satisfy both. `Box<dyn Connection>`
  is the only place a non-`Sized` type appears at the API boundary.
- Memory strategy: pool holds at most `max_open` live connections plus a
  bounded idle queue. `druid-stats` retains one histogram per unique
  SQL fingerprint with a TTL configurable via `druid-stats::MergeConfig`.
- Resource release: `PooledConnection::drop` is the single point that returns
  the connection to the pool and emits leak warnings.
- Cancellation: every `async` method takes `&mut self` and accepts a
  caller-driven `tokio::select!` race; no internal spawn is created from a
  caller context.
- Backpressure: `pool.get()` blocks when `in_use == max_open`; the optional
  `acquire_timeout` returns `Error::AcquireTimeout` instead of waiting
  indefinitely.

## 11. Roadmap and Phases

The phases below align with `doc/5、druid-rust-技术方案与路线.md` and
`druid-rust-Architecture.zh_CN.md` §23.

| Phase | Deliverable | Exit condition | Dependency | Risk |
| :---: | :--- | :--- | :--- | :--- |
| Phase 0 | Workspace skeleton + design docs (current) | `cargo check --workspace` passes | toolchain | dependency drift |
| Phase 1 | `druid-core` + `druid-sql` + `druid-pool` + mock driver | `SELECT 1` end-to-end; `DROP TABLE` blocked by Wall | Phase 0 | API drift |
| Phase 2 | `druid-rbdc` + `druid-sqlx-deadpool` + `druid-sqlx-bb8` + `druid-stats` | Prometheus exporter live; one driver adapter smoke test | Phase 1 | upstream `sqlx` / `rbdc` changes |
| Phase 3 | `druid-dynamic` + `druid-admin` | Hot switch demo; `/druid/admin` JSON endpoints | Phase 2 | ArcSwap semantics validation |

> Time estimates are deliberately omitted. Replace them with verifiable exit
> conditions before they are quoted in release notes.

## 12. Planned Documentation Set

| Document | Purpose |
| :--- | :--- |
| `druid-rust-Architecture.zh_CN.md` | Workspace, crate, key decisions (canonical architecture baseline) |
| `doc/` (root) | 10 product-level documents aligned with the `full-stack-doc` v3 standard |
| `doc/V1/` | 7 version-level documents for the Phase 1 milestone |
| `LICENSE` | Apache-2.0 |

## 13. Planned Quality Gates

The gates below are **targets**, not current reality. None of them run in
CI today.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-features
cargo test --workspace
cargo test --workspace --no-default-features
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo llvm-cov --workspace --all-features
cargo audit
cargo deny check
```

## 14. Planned Benchmark Surface

| Scenario | Goal | Status |
| :--- | :--- | :--- |
| `pool_acquire` | median < 200ns on a warm pool | not yet measured |
| `sql_merge` | parameterize + fingerprint under 50µs | not yet measured |
| `wall_check` | full Wall check under 100µs | not yet measured |
| `dynamic_switch` | `ArcSwap::store` followed by `load` under 50ns | not yet measured |

> Benchmark claims will only be published alongside reproducible scripts,
> pinned hardware and `git rev-parse HEAD` references.

## 15. Planned Compatibility and Migration

| Topic | Target |
| :--- | :--- |
| SemVer | crates follow SemVer once `publish = false` is removed |
| MSRV policy | MSRV bumps require a minor version bump after Phase 1 |
| Default features | changing defaults is a breaking change |
| Source compatibility | tracked against `druid-rust-Architecture.zh_CN.md` §6 ADRs |

## 16. Troubleshooting

The placeholder workspace intentionally has no troubleshooting table yet.
If you observe unexpected behaviour while reading the design documents,
open an issue with the label `design-question` and link to the offending
section.

## 17. Planned crates.io Publication

No crate is currently published. The publication prerequisites are:

- [ ] `druid-core` exports its full trait surface and is covered by unit tests.
- [ ] `druid-pool` is benchmarked against `deadpool` and `bb8` for the warm
      acquire path.
- [ ] `cargo publish --dry-run` succeeds for every crate with `publish = false`
      removed.
- [ ] CI runs `cargo fmt`, `cargo clippy -D warnings`, `cargo test` and
      `cargo audit` on every PR.
- [ ] docs.rs builds succeed against the published feature matrix.

## 18. Planned Contribution Flow

- Open an issue or discussion before opening a PR for new crates or ADRs.
- Run the planned gates listed in §13 before requesting review.
- New public API must come with docs, an example, a test, and a SemVer /
  MSRV impact note in the PR description.

## 19. Contributing, Security and License

druid-rust is licensed under [Apache-2.0](LICENSE).

Vulnerability reports follow the project's private disclosure channel,
which will be created before the first release.

---

<div align="center">

[Back to top](#readme-top) · [Architecture](druid-rust-Architecture.zh_CN.md) ·
[Product docs](doc/) · [Issues](https://github.com/easy-4-rust/druid-rust/issues)

</div>
