# 快速开始

## 环境要求

- Rust **1.95+**（仓库 `rust-toolchain.toml` 锁定 1.97.1，MSRV 1.95）
- 迁移进行中，所有 crate 均为 `publish = false`，以 **Git 依赖**接入

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
druid = { git = "https://github.com/easy-4-rust/druid-rust.git", branch = "main" }
# 需要适配层时（SQLx/RBDC/bb8/deadpool/DuckDB/libSQL/JDBC Agent）：
# druid-wrapper = { git = "https://github.com/easy-4-rust/druid-rust.git", branch = "main" }
```

## 第一个例子：内置 Toasty SQLite 链路

`druid` 内置 Toasty 数据源，零外部数据库即可跑通 **标准 trait 树**（`druid-rdbc`）全链路：

```rust
use druid_wrapper::toasty_adapter::ToastyConnectionFactory;

let mut factory = ToastyConnectionFactory::new("sqlite::memory:")
    .await
    .expect("Toasty SQLite factory");
let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
    .await
    .expect("standard connection");
let mut prepared = connection
    .prepare_statement("select ?", druid_rdbc::statement::StatementOptions::default())
    .await
    .expect("prepared statement");
prepared.set_parameter(/* Value::String("toasty-standard") */)?;
let mut rows = prepared.execute_prepared_query().await?;
assert!(rows.next().await.unwrap());
```

> 示例改编自仓库测试 `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`。

## 运行 XA / 两阶段提交示例

仓库自带 XA 演示，实现 `XaResource` 即可让自定义资源参与 2PC：

```rust
use druid::core::xa::{flags as xa_flags, XaPrepareResult, XaResource, XaState, XaTransactionState, Xid};

#[async_trait::async_trait]
impl XaResource for InMemoryLedger {
    async fn start(&mut self, xid: &Xid, _flags: i32) -> Result<(), druid::core::DruidError> {
        // 登记分支事务
    }
    // end / prepare / commit / rollback ...
}
```

```bash
cargo run -p druid --example xa_demo
```

## 驱动管理 CLI

`druid-admin` 提供驱动目录与安装管理：

```bash
cargo run -p druid-admin --bin druid-driver -- catalog     # 查看驱动目录
cargo run -p druid-admin --bin druid-driver -- doctor      # 环境诊断
```

## Cargo Features

| crate | feature | 说明 |
| --- | --- | --- |
| druid | `sqlite` / `postgresql` / `mysql` / `turso` | Toasty 内置数据源 |
| druid-wrapper | `mysql-native` / `sqlserver-native` / `duckdb-native` / `libsql-native` / `http-sql` / `jdbc-agent` | 原生驱动与长尾接入 |
| druid-wrapper | `all-databases` | 一键启用全部 |

## 下一步

- 阅读[架构与 Crate 组成](/guide/architecture)了解 workspace 拓扑；
- 阅读[功能详解](/guide/features)查看驱动支持矩阵；
- 查看[仓库 README](https://github.com/easy-4-rust/druid-rust) 的迁移路线（P0–P10）。
