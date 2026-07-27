# druid-rust V1 功能与界面规划

> **文档说明**：V1 范围内模块划分与界面（端点）规划，作为 PRD 与开发
> 任务拆分的输入。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

### 1.1 版本记录

| 版本 | 日期 | 作者 | 变更说明 |
| :--- | :--- | :--- | :--- |
| V1.0.0 | 2026-07-27 | druid-rust maintainers | 初始规划 |

### 1.2 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [V1/2、需求分析](2、druid-rust-需求分析文档-V1.md) | 用户故事 |
| [V1/5、PRD](5、druid-rust-PRD文档-V1.md) | 需求规格 |
| [架构文档 §8](../../druid-rust-Architecture.zh_CN.md) | 组件映射 |

---

## 2. V1 模块清单

| 模块 | Crate | 职责 | 接口 |
| :--- | :--- | :--- | :--- |
| Core | `druid-core` | trait 契约 | `Connection`、`Driver`、`Pool`、`Filter`、`ConnectionFactory` |
| SQL | `druid-sql` | AST 解析、Wall、参数化、指纹 | `WallConfig`、`ParsedStmt` |
| Pool | `druid-pool` | 池调度、归还、泄漏 | `DruidPool`、`PooledConnection` |
| Mock driver | `tests/`（临时） | 验证主链 | 实现 `ConnectionFactory` |

## 3. 界面（端点）规划

V1 **不提供**任何 HTTP 端点；`druid-admin` 在 V3 才引入。V1 的"界面"
仅指 trait API：

```rust
// druid-core
pub trait Connection: Send + Sync {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, Error>;
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, Error>;
    async fn begin(&mut self) -> Result<(), Error>;
    async fn commit(&mut self) -> Result<(), Error>;
    async fn rollback(&mut self) -> Result<(), Error>;
    async fn ping(&mut self) -> Result<(), Error>;
    async fn close(&mut self) -> Result<(), Error>;
}

pub trait Pool: Send + Sync {
    async fn get(&self) -> Result<PooledConnection, Error>;
    async fn get_timeout(&self, d: Duration) -> Result<PooledConnection, Error>;
    async fn state(&self) -> PoolState;
    fn driver_name(&self) -> &str;
}

// druid-sql
pub struct WallConfig { /* see §4 */ }
pub struct ParsedStmt { /* see §4 */ }

// druid-pool
pub struct DruidPool { /* opaque */ }
pub struct PooledConnection { /* opaque, drop returns conn */ }
```

## 4. V1 配置合同

```rust
// druid-sql
let wall = WallConfig::default()
    .deny_drop_table(true)
    .deny_truncate(true)
    .update_where(WherePolicy::Required)
    .delete_where(WherePolicy::Required)
    .max_join_tables(Some(8))
    .max_subquery_depth(Some(4))
    .max_sql_length(Some(64 * 1024))
    .build();

// druid-pool
let pool = DruidPool::builder()
    .factory(Arc::new(MockFactory::new()))
    .max_open(20)
    .max_idle(4)
    .acquire_timeout(Duration::from_secs(3))
    .filter_chain(Arc::new(FilterChain::new(vec![
        Arc::new(WallFilter::new(wall.clone())),
        Arc::new(LogFilter::default()),
    ])))
    .build()
    .await?;
```

## 5. V1 不在范围内

- 三个 adapter crate（V2）
- `druid-stats` SQL 合并（V2）
- `druid-dynamic` 多数据源（V3）
- `druid-admin` HTTP 端点（V3）
- Web UI（V3 之后单独评估）

## 6. 一致性自检清单

- [ ] 模块清单与 [架构文档 §8](../../druid-rust-Architecture.zh_CN.md) 一致。
- [ ] 配置合同与 [架构文档 §14](../../druid-rust-Architecture.zh_CN.md) 一致。
- [ ] V1 不在范围内的事项与 [V1/2、需求分析](2、druid-rust-需求分析文档-V1.md) §3.2 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审