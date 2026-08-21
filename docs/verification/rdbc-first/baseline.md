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
- public API 快照：`NOT_CAPTURED`。
- 原始命令：`cargo public-api -p druid -sss --all-features > api/druid-public-api.rdbc-first.before.txt`
- 原始失败（退出码 1）：

```text
error: toolchain 'nightly-aarch64-apple-darwin' is not installed
help: run `rustup toolchain install nightly-aarch64-apple-darwin` to install it
Error: `"Cargo.toml"` is a virtual manifest.

Listing or diffing the public API of an entire workspace is not supported.

Try

    cargo public-api -p specific-crate
```

本机安装状态为 `cargo-public-api 0.52.0`、`cargo 1.97.1`，活跃工具链为
`1.97.1-aarch64-apple-darwin`；没有安装 nightly。失败命令产生的 0 字节文件
未作为 API 快照保留，因此 SHA-256 为 `NOT_AVAILABLE`。在明确提供/授权所需
nightly 后，必须重新运行上述三条 public API/SHA-256 命令并更新本基线，再开始
任何依赖 API baseline 的结构迁移。
