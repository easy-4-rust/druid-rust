# 迁移治理规格

> 日期：2026-08-12  来源：原 docs/README.md 全局治理段

## 状态定义

| 状态 | 定义 |
| :--- | :--- |
| `PASS` | Java oracle 与 Rust 差分通过，真实依赖路径也通过 |
| `PARTIAL` | 一部分场景通过，但配置、分支或调用入口不完整 |
| `FAIL` | 已存在实现，但证据证明语义错误 |
| `TODO` | 尚未实现或尚无差分证据 |

单元测试通过不自动得到 `PASS`。每条契约至少记录：

```text
contract_id
java_source + java_fixture
rust_source + rust_test
input_matrix
expected_result / expected_error
state_before / state_after
side_effects (metrics, logs, callbacks)
concurrency_and_cancellation
mapping_decision
```

## 差分评测流水线

```mermaid
flowchart LR
    Corpus["版本化输入语料<br/>配置/SQL/时序/故障"] --> Java["Java Druid 1.2.28 Oracle"]
    Corpus --> Rust["druid-rust Candidate"]
    Java --> Normalize["规范化器<br/>时间、ID、路径、错误链"]
    Rust --> Normalize
    Normalize --> Diff["字段级/事件序列/状态机差分"]
    Diff --> Pass["PASS + 证据归档"]
    Diff --> Fail["FAIL + 最小反例"]
    Fail --> Ledger["更新对象账本/语义账本"]
```

规范化只允许消除随机 ID、绝对时间和平台路径等非语义噪声，不得抹平错误类型、SQL 输出、统计字段或事件顺序。

## SEM-NFR-* 表

这些 ID 约束三个模块共同通过，不归任一单独 crate 冒充完成。

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-NFR-001 | 错误类别、cause、上下文字段 | PARTIAL | error snapshot |
| SEM-NFR-002 | 不 panic；错误可恢复 | PARTIAL | fuzz/fault injection |
| SEM-NFR-003 | maxActive、active/pooling/total 守恒 | FAIL | loom + stress |
| SEM-NFR-004 | async cancellation 安全 | TODO | 每 await 点取消 |
| SEM-NFR-005 | shutdown 不丢连接/任务 | TODO | graceful timeout |
| SEM-NFR-006 | 配置/规则热更新一致性 | TODO | versioned snapshot |
| SEM-NFR-007 | 密钥和 SQL 参数不泄漏 | TODO | log/metric scan |
| SEM-NFR-008 | 长稳无内存/连接/任务泄漏 | TODO | 24h soak |
| SEM-NFR-009 | 性能不低于发布预算 | TODO | Java/Rust 同环境 benchmark |

## 名称规范

| Java | Rust | 规则 |
| :--- | :--- | :--- |
| `DruidDataSource` | `DruidDataSource` / `druid_data_source.rs` | 直接迁移保留类型语义名 |
| `JdbcSqlStat` | `JdbcSqlStat` / `jdbc_sql_stat.rs` | `Jdbc` 是源对象身份，不随意删除 |
| `SQLUtils` | `SqlUtils` / `sql_utils.rs` | acronym 按 Rust 风格规范化 |
| `JDBC4ValidConnectionChecker` | `Jdbc4ValidConnectionChecker` | 数字与 acronym 保留语义 |
| `PGExceptionSorter` | `PgExceptionSorter` | acronym 规范化 |
| `MySqlExceptionSorter` | `MySqlExceptionSorter` | Java 已使用 MySql |
| `DruidXADataSource` | `DruidXaDataSource` | acronym 规范化 |
| `DruidDataSourceC3P0Adapter` | `DruidDataSourceC3p0Adapter` | 数字保留，字母按 Rust PascalCase 规范化 |
| `getConnection` | `get_connection` | 方法 snake_case，语义词不删减 |
| `maxActive` | `max_active` | 字段 snake_case |

允许省略 `Druid`/`Jdbc` 前缀的仅限明显的 Rust 内部实现对象；它不能代替 canonical 迁移 facade。`java.sql.Connection`、`java.sql.Driver` 等平台标准不属于 Druid canonical 对象，因此不机械保留 Java 类型名；其 Rust 目标使用能表达边界的 `PhysicalConnection`、`PhysicalConnectionFactory`，并在平台依赖映射表中追踪。

## 文件组织检查

1. 每个 Java 直接迁移对象对应一个 `.rs` 文件。
2. 内部 Builder 可与主对象同文件；独立 Java Builder 仍需独立账目。
3. `mod.rs`、`lib.rs` 只做声明和 `pub use`。
4. Java 最后一级子包映射到同语义 Rust 目录。
5. 生产代码禁止 wildcard import。
6. 每个迁移对象与公开方法均有中文 doc 注释并注明 Java FQCN/方法。
7. `MERGE` 的每个 Java 对象必须在 enum variant 或 compatibility type 注释中单独出现。

## 原"Java 有、Rust 无"分类的修订

| 原分类 | 原数量/说法 | 修订结论 |
| :--- | :--- | :--- |
| SQL parser | 1,268 个"不需要迁移" | 全部进入对象账本；可映射到 type/variant/adapter |
| JDBC Proxy | 36 个"不需要迁移" | 迁移为 async proxy/context/driver adapter |
| Mock | 22 个"不需要迁移" | 登记为 `TEST_ADAPTER` fixture |
| Support/Spring | 105 个"不需要迁移" | 迁移业务结果到 Axum/Tower/config/tracing |
| XA | 4 个"不需要迁移" | 迁移 2PC 状态机与 capability error |
| HA/ZooKeeper | 监听对象"不需要迁移" | 迁移到 Registry/Watcher SPI |
| MBean | 42 个接口"不需要迁移" | 映射 OpenMetrics descriptor + admin operation |

对象可以没有同名公开 Rust struct，但不能没有目标对象、协议结果和验收契约。

## 自动检查设计

### 输入

- Java CodeGraph 类型清单：FQCN、kind、source path、public methods；
- Rust CodeGraph 类型清单：crate、type、source path、public methods；
- 对象账本：mapping kind、canonical target、ADR、status。

### 规范化

```text
Java type PascalCase -> Rust type PascalCase（acronym: SQL→Sql, JDBC→Jdbc, XA→Xa, PG→Pg）
Java source FooBar.java -> Rust source foo_bar.rs
Java method lowerCamel -> Rust method snake_case
```

### CI 失败条件

1. Java 基线对象未登记；
2. `DIRECT` 对象没有 canonical Rust 类型或文件；
3. `MERGE/SPLIT/ADAPTER/PROTOCOL` 缺 ADR 或契约 ID；
4. 同一 Rust 文件定义多个无关迁移对象；
5. `mod.rs/lib.rs` 定义业务对象；
6. 公开方法缺中文来源注释；
7. 生产代码存在 wildcard import、`todo!()`、`unimplemented!()` 或"not yet implemented"；
8. 文档标 `DONE` 但路径/类型/测试不存在。

建议检查命令：

```bash
rg --files core/src/main/java -g '*.java'
rg --files crates -g '*.rs'
rg -n '^pub (struct|enum|trait|type) ' crates -g '*.rs'
rg -n 'todo!|unimplemented!|not yet implemented|use .+::\\*' crates -g '*.rs'
cargo +stable test --workspace
cargo +stable clippy --workspace --all-targets -- -D warnings
```

## 通过标准

命名一致性不是追求 1,719 个 Rust struct，而是实现三项 100%：

- Java 对象登记率 100%；
- `DIRECT` canonical 名称合规率 100%；
- 非一对一映射的 ADR、目标对象和语义契约绑定率 100%。

在此之前，不得使用"对象名称一致""核心逻辑 100%"等结论。

## 对象关闭条件

一个对象只有同时满足以下条件才可标 `DONE`：

- Java FQCN、源提交、Rust 类型和文件均已登记；
- 映射形态及必要 ADR 已批准；
- 字段默认值、公开方法、错误、副作用和并发语义有契约；
- Java fixture 与 Rust 测试差分通过；
- 中文 doc 注释注明 Java 来源；
- 名称检查、无 stub 检查、CodeGraph 调用链检查通过；
- 若由 enum 承载，源对象对应的 variant 不与其他对象混账。

## 测试基线（保留并升级原验收）

原 `druid-core` 等测试已并入 `druid`；adapter 合同已并入 `druid-wrapper`。

| 当前模块/对象域 | 归并来源（历史） | 原验收 | 修订验收 |
| :--- | :--- | :--- | :--- |
| `druid/core` | `druid-core` | 每 trait 一个 mock | 方法/错误差分 + 真实 adapter |
| `druid/pool` | `druid-pool` | 10,000 acquire/release | 加 loom、取消、泄漏、状态快照 |
| `druid/sql` | `druid-sql` | DROP/DELETE/UPDATE 三类 | 全方言 corpus + AST/output/parameterize |
| `druid/stats` | `druid-stats` | SQL 合并率 ≥90% | Java key 和快照逐字段一致 |
| `druid/dynamic` | `druid-dynamic` | 1,000 RPS 错误率 <1% | 加事务粘性、恢复、排空 |
| `druid-admin` | `druid-admin` | datasource JSON 可访问 | 全端点 schema、认证、IP、reset |
| `druid-wrapper` | 原四个 adapter/bridge crate | 任一 Postgres 冒烟 | 所有 adapter 真实数据库矩阵 |

## 迁移门禁

每个阶段关闭前运行：

1. Java fixture 生成或执行 oracle。
2. Rust 同输入执行候选实现。
3. 结果、错误、状态快照、事件序列、指标逐字段比较。
4. 并发契约使用 loom/压力测试，驱动契约使用真实数据库。
5. 未绑定对象、未绑定契约、stub、仅 mock adapter、失效配置字段任一存在即失败。
