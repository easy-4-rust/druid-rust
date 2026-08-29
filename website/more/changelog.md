# 更新日志与路线图

druid-rust 目前**没有独立 CHANGELOG 文件**——项目处于 `0.0.0-design` 迁移期，变更记录以下列仓库内载体为准，发布首个版本（计划发布 `druid` / `druid-wrapper` / `druid-admin` 三个 crate）后将建立正式 CHANGELOG。

## 权威信息源

| 载体 | 内容 |
| --- | --- |
| [README §11 迁移路线](https://github.com/easy-4-rust/druid-rust/blob/main/README.zh-CN.md) | P0–P10 阶段路线表 |
| `docs/superpowers/plans/` | 26 个带 checkbox 的分日执行计划（2026-08-12 起） |
| `docs/verification/` | 各阶段验收报告（约 30 个证据文件） |
| [GitHub Releases](https://github.com/easy-4-rust/druid-rust/releases) | 发布记录 |

## 近期里程碑（摘自验证报告）

- **RDBC 标准 trait 树契约 v1 定稿**，physical → standard 迁移完成
- **Phase 1 四驱动 conformance 全绿**：tiberius（SQL Server）、mysql_async、Toasty PostgreSQL、Toasty Turso —— 9/9
- **JDBC Agent 验证**：H2 2.3.232 / HSQLDB 2.7.4 × 3 OS × JDK 17/21
- **public-api 快照 + semver-checks** 门禁上线，MSRV 1.95

## 发布前检查清单

README §17 维护着 crates.io 发布前检查清单，核心项包括 API 冻结、文档补全与三 crate 发布范围确认。
