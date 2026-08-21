# RDBC-First 执行基线

本文件冻结 ADR-CRATE-002 执行前的只读事实。后续计划必须保留本文件中的
`BASELINE_SHA`，并在各自报告中记录新鲜命令输出；它不表示任何 RDBC 迁移已经完成。

## 仓库

- `BASELINE_SHA`: `8e97a5e7e9b86be53583a7e01f8228d059cb9dde`
- 分支：`main...origin/main`
- 工作区：包含已经批准、将与本基线一同提交的 ADR-CRATE-002 规格、RDBC-First
  计划、架构导航和旧计划 supersede 标记；没有生产 Rust 或 Java 代码改动。

## Cargo package 集合

`cargo metadata --format-version 1 --no-deps | jq -r '.packages[].name' | sort`：

```text
druid
druid-admin
druid-bench
druid-metrics
druid-wrapper
```

## CodeGraph

`codegraph status`：索引为 `up to date`；694 files、16,583 nodes、65,189 edges，
SQLite WAL 后端。

## 测试与 public API

- `cargo test --workspace`：退出码 0；workspace、集成测试和 doctest 均通过。
- public API 快照：`api/druid-public-api.rdbc-first.before.txt`，非空，共 31,634 行，
  首行是 `pub mod druid`，不是错误捕获。
- 工具链：`nightly-2026-08-07`，`rustc 1.99.0-nightly (84b36a78a 2026-08-06)`；
  `cargo-public-api 0.52.0`。
- 原始成功命令：

```bash
RUSTUP_TOOLCHAIN=nightly-2026-08-07 cargo public-api -p druid -sss --all-features > api/druid-public-api.rdbc-first.before.txt
```

- `shasum -a 256 api/druid-public-api.rdbc-first.before.txt`：
  `bf88754fefc9c57ae1519e93f70f0db7118747dc3a1334156ec93778beac94b6`。

早期未安装 nightly 时的 `cargo public-api` 尝试曾以退出码 1 失败；在 controller
明确授权并安装指定 nightly 后，上述命令以退出码 0 完成。API baseline gate 已满足；
`druid-rdbc-extraction` 仍只等待 P01 证据。
