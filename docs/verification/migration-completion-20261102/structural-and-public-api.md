# 6.4 结构债与 public-api/SemVer 复核

日期：2026-11-02
基线：`66dbd173` + `592504bc`（Wall 内核 API 快照与 SemVer 映射修复）

## public API 快照

CI `public-api` 作业在 `aarch64-apple-darwin` 上用 `cargo-public-api 0.52.0`、nightly-2026-08-07 生成并上传观测工件。本批用该工件刷新 `api/*.single-standard.after.txt` 与 `api/public-api.aarch64-apple-darwin.sha256`：

| crate | SHA-256 |
|---|---|
| druid-rdbc | `7ab08cdead9e17f32e1bfb315d44145303fc5aeaa9b004e2f3cb1e306951339f` |
| druid | `1ddaebf89d9df4613c29a65dc4eff12e46e3d7f3f65666a8e80ad2a3d78d60cf` |
| druid-wrapper | `a6fca9c0e35a0fc572ff8f47a516e9ec7ae7296976964ea22622630e783cde0e` |

变更全部是 Wall 内核下沉的预期结果：`WallConfig`、`WallConfigBuilder`、`TenantCallBack`、`TenantStatementType`、`WallUpdateCheckHandler` 从 `druid::sql::*` 的定义位置迁到 `druid_rdbc::governance::wall::*`，`druid::sql::*` 继续重导出。用 `diff` 核对，`druid` 快照的 382 行删除、98 行新增全部命中 `Wall|Tenant|wall_config|tenant`，无其他公共 API 漂移。

## SemVer 报告

`cargo-semver-checks` 对 `druid-rdbc`、`druid`、`druid-wrapper` 均以 exit 100 报告预发布破坏；经 `scripts/verify_rdbc_single_standard_breaking_api.py` 精确比对：

- 30 个已批准破坏类别全部命中 allowlist 的 count + SHA-256 指纹；
- `druid/struct_missing` 新增 `WallConfig`、`WallConfigBuilder`，已在 `api/druid-struct-migration.tsv` 绑定到 `crates/druid-rdbc/src/governance/wall/wall_config.rs`；
- `druid/enum_missing` 新增 `TenantStatementType`，`druid/trait_missing` 新增 `TenantCallBack`、`WallUpdateCheckHandler`，均属于同一内核迁移；指纹已刷新并保留批准理由；
- 复核命令 exit 0，输出 `Verified 30 exact approved SemVer breaking categories`。

## 结构债

| 检查 | 结果 |
|---|---|
| EX-002 `crates/druid-rdbc/src/extensions/row_set.rs` | 1,128 行，与 2026-10-23 批准记录一致，未增长 |
| EX-003 `crates/druid-rdbc/src/result_set/result_set_ext.rs` | 1,216 行，与批准记录一致，未增长 |
| 其他超过 800 行的 authored `.rs` | 0 个（对 `crates/*/src/**/*.rs` 全量计数） |
| 例外复审日期 | `docs/migration/p6-structural-exceptions.md` 保留 **2026-12-31** |

结论：6.4 的 public-api/SemVer 与结构债检查通过；两项例外仍在有效期内，未出现新增超 800 行文件。该结论只覆盖本任务，不外推全仓迁移完成。
