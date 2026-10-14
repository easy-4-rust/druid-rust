# CommonDataSource 配置与迁移

`druid-rdbc::data_source::CommonDataSource` 是普通数据源、连接池数据源和 XA 数据源共享的标准配置合同。每个数据源实例拥有自己的日志 writer；`None` 表示关闭诊断输出。诊断只写固定操作名、错误类别和有效 SQLState，不包含原始错误消息或连接凭据。

## 构造 writer

`RdbcLogWriter` 的公开类型使用 `parking_lot::Mutex`，应用需在自己的 `Cargo.toml` 中直接声明 `parking_lot`，不要依赖传递依赖：

```toml
[dependencies]
druid-rdbc = "0.0.0-design"
parking_lot = "0.12"
```

```rust
use druid_rdbc::data_source::RdbcLogWriter;
use parking_lot::Mutex;
use std::{io::Write, sync::Arc};

fn diagnostic_writer() -> RdbcLogWriter {
    Arc::new(Mutex::new(Box::new(std::io::stderr()) as Box<dyn Write + Send>))
}
```

## 池化数据源与普通 RDBC 数据源

在建池阶段配置以秒为单位的登录超时。池构建完成后配置日志 writer；普通 wrapper 可复用同一个 canonical 池，不会额外创建一层连接池。

```rust
use druid::pool::DruidPoolBuilder;
use druid_rdbc::data_source::CommonDataSource;
use druid_wrapper::rdbc::DruidRdbcDataSource;
use std::sync::Arc;

async fn build_data_sources(
    factory: Arc<dyn druid::core::ConnectionFactory>,
) -> Result<DruidRdbcDataSource, Box<dyn std::error::Error>> {
    let pooled = DruidPoolBuilder::new()
        .name("orders")
        .driver_name("postgresql")
        .factory(factory)
        .login_timeout(15)
        .build_data_source()
        .await?;

    CommonDataSource::set_log_writer(&pooled, Some(diagnostic_writer()))?;
    let ordinary = DruidRdbcDataSource::new(pooled);
    CommonDataSource::set_log_writer(&ordinary, Some(diagnostic_writer()))?;
    Ok(ordinary)
}
```

如果调用方已经通过驱动目录构建数据源，可用 `DruidDatabasePoolBuilder::from_rdbc_url(...).pool(|pool| pool.login_timeout(15)).build_data_source().await` 在驱动解析时完成同一配置。数据库凭据通过 builder 的独立用户名/密码入口或秘密管理系统提供，不要放入 URL 查询参数或日志。

读取标准登录超时使用 `try_login_timeout()`：

```rust
use druid_rdbc::data_source::CommonDataSource;
use std::time::Duration;

fn timeout(source: &dyn CommonDataSource) -> Result<Option<Duration>, druid_rdbc::error::RdbcError> {
    source.try_login_timeout()
}
```

`None` 表示实现没有显式配置；`Some(Duration::ZERO)` 表示无限等待；正值保留 `Duration` 精度。Druid pool 的 Java 兼容 builder 使用整数秒，其值通过标准 getter 暴露；负值返回 `InvalidArgument`，不会被截断或归一化。已构建数据源的 `set_login_timeout` 返回 `UnsupportedOperation`；请在 builder 上设置，不能只更新展示值。

## XA 数据源

XA 必须显式指定厂商 `XADataSource` 类及 Java Agent 配置。调用方在构造时提供厂商支持的连接超时属性；属性名、单位和行为由厂商决定。之后可通过同一个 `CommonDataSource` 合同设置日志 writer。完整 PostgreSQL Agent 配置见 [JDBC Agent XA 集成](../jdbc-agent-xa.md)。

```rust
use druid_rdbc::data_source::{CommonDataSource, RdbcLogWriter};
use druid_wrapper::jdbc_agent::{JdbcAgentOptions, JdbcAgentXaDataSource};
use parking_lot::Mutex;
use serde_json::Map;
use std::{io::Write, sync::Arc};

fn configure_xa_diagnostics(
    options: JdbcAgentOptions,
) -> Result<JdbcAgentXaDataSource, druid_rdbc::error::RdbcError> {
    let source = JdbcAgentXaDataSource::new(
        "org.postgresql.xa.PGXADataSource",
        Map::new(),
        options,
    )?;
    let writer: RdbcLogWriter = Arc::new(Mutex::new(
        Box::new(std::io::stderr()) as Box<dyn Write + Send>
    ));
    CommonDataSource::set_log_writer(&source, Some(writer))?;
    Ok(source)
}
```

应用不需要 XA 时，继续使用普通 `DataSource`；不要根据 URL 或普通 JDBC Agent 连接推断 XA 能力。XA 的 owner、恢复和迁移边界见 [XA 集成指南](../jdbc-agent-xa.md)。

## 从旧 getter 迁移

| 旧调用 | 新调用 | 行为 |
|---|---|---|
| `source.login_timeout()` | `source.try_login_timeout()?` | 旧标准 getter 已弃用；新入口保留读取错误。 |
| 忽略 getter 错误并用零值 | 显式处理 `Err` | 不把失败误报成无限等待或默认值。 |
| 构建后调用 `set_login_timeout` | 在池 builder 上调用 `.login_timeout(seconds)` | 已构建池不可变；失败以 `UnsupportedOperation` 返回。 |
| `druid::sql::CommonDataSource` 产品接口 | `druid_rdbc::data_source::CommonDataSource` 标准接口 | 产品接口为兼容门面；两者不是同一个 trait。调用重名方法时使用完整 trait 路径。 |
| 设置进程级 `DriverManager` writer | 设置目标数据源实例的 writer | 数据源诊断按 owner 隔离；`DriverManager` writer 仍只用于其自身驱动入口。 |

## 外部消费者验证

仓库的公开消费者测试构建一个不建立物理连接的池，验证池及普通 wrapper 的 `try_login_timeout`、`set_log_writer`、writer 身份、不可变配置错误、零值与负值语义，并验证 XA writer 和父命名空间。运行：

```powershell
cargo test -p druid-wrapper --features jdbc-agent --test common_data_source_public_examples_test --locked --offline
cargo test -p druid-rdbc --test common_data_source_timeout_contract_test --locked --offline
cargo test -p druid --test common_data_source_logging_pool_test --locked --offline
cargo test -p druid-wrapper --test common_data_source_logging_test --locked --offline
```
