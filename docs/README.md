# druid-rust 文档总入口

本目录定义 Druid 1.2.28 到 druid-rust 的功能语义完整迁移基线。项目级
路线、架构和三个模块账本已经归入以下唯一入口：

| 文档 | 职责 |
| :--- | :--- |
| [迁移总路线图](./迁移总路线图.md) | 跨模块阶段、依赖、风险和统一门禁 |
| [总体架构](./druid-rust-Architecture.zh_CN.md) | 三模块职责、依赖方向和部署边界 |
| [连接抽象与驱动适配架构](./连接抽象与驱动适配架构.md) | `DruidPooledConnection`、`PhysicalConnection`、native pool 与 external bridge |
| [`druid` 账本](./druid/迁移路线图.md) | core 路线、对象、语义、名称和 Toasty 内置实现 |
| [`druid-admin` 账本](./druid-admin/迁移路线图.md) | 管理端路线、对象、语义和名称 |
| [`druid-wrapper` 账本](./druid-wrapper/迁移路线图.md) | 扩展驱动与外部池路线、对象、语义和名称 |

统一口径：这是迁移，不是借鉴；允许 Rust 化实现，但不允许丢失功能语义。

## 三模块治理基线

产品、发布与依赖治理边界只有三个：

| 模块 | Java 来源与职责 |
| :--- | :--- |
| `druid` | Java `/core` 的完整语义迁移；内部包含 core、pool、SQL/Wall、Stat、Dynamic 和默认 Toasty 数据源实现 |
| `druid-admin` | Java `/druid-admin` 的管理、监控、认证与 API 语义迁移 |
| `druid-wrapper` | Java `/druid-wrapper` 及 Rust 数据库生态封装；内部包含 SQLx、RBDC、bb8、deadpool |

原 `druid-core`、`druid-pool`、`druid-sql`、`druid-stats`、`druid-dynamic`、
`druid-toasty`、`druid-sqlx`、`druid-rbdc`、`druid-sqlx-bb8`、
`druid-sqlx-deadpool` 已物理迁入 `druid/src/*` 或
`druid-wrapper/src/*`，独立目录和 workspace member 已删除。

## 文档维护原则

- 原文中的对象清单、字段映射、阶段目标文件、工作量、风险和覆盖率记录应保留。
- 发现失实内容时，增加“当前证据、修订目标、验收条件”，不直接删除有价值的明细。
- “目标文件”与“当前已实现”必须分列，计划不得使用完成状态。
- Java 生态能力使用 `ADAPTER` 或 `PROTOCOL` 迁移，不以运行时形态不同为由移出范围。
- 删除迁移条目必须有 ADR、替代语义和评审记录。
- 迁移文档中的模块归属只能使用三个产品模块；提到已删除的原 crate 时必须标注为历史证据。

<!-- GLOBAL_GOVERNANCE_MERGE_START -->
## 全局迁移治理

原全局总账中不属于单一模块的规则归入本入口；模块对象、语义和名称状态只在
三个模块目录维护，全 workspace 非功能门禁只在本入口维护。

### 1. 状态与验收规则

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

### 2. 差分评测流水线

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

### 全 workspace 非功能语义 ID

#### `SEM-NFR-*`

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

### 18. 测试基线（保留并升级原验收）

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

### 19. 迁移门禁

每个阶段关闭前运行：

1. Java fixture 生成或执行 oracle。
2. Rust 同输入执行候选实现。
3. 结果、错误、状态快照、事件序列、指标逐字段比较。
4. 并发契约使用 loom/压力测试，驱动契约使用真实数据库。
5. 未绑定对象、未绑定契约、stub、仅 mock adapter、失效配置字段任一存在即失败。

### 2. 名称规范

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

### 3. 文件组织检查

1. 每个 Java 直接迁移对象对应一个 `.rs` 文件。
2. 内部 Builder 可与主对象同文件；独立 Java Builder 仍需独立账目。
3. `mod.rs`、`lib.rs` 只做声明和 `pub use`。
4. Java 最后一级子包映射到同语义 Rust 目录。
5. 生产代码禁止 wildcard import。
6. 每个迁移对象与公开方法均有中文 doc 注释并注明 Java FQCN/方法。
7. `MERGE` 的每个 Java 对象必须在 enum variant 或 compatibility type 注释中单独出现。

### 9. 原“Java 有、Rust 无”分类的修订

| 原分类 | 原数量/说法 | 修订结论 |
| :--- | :--- | :--- |
| SQL parser | 1,268 个“不需要迁移” | 全部进入对象账本；可映射到 type/variant/adapter |
| JDBC Proxy | 36 个“不需要迁移” | 迁移为 async proxy/context/driver adapter |
| Mock | 22 个“不需要迁移” | 登记为 `TEST_ADAPTER` fixture |
| Support/Spring | 105 个“不需要迁移” | 迁移业务结果到 Axum/Tower/config/tracing |
| XA | 4 个“不需要迁移” | 迁移 2PC 状态机与 capability error |
| HA/ZooKeeper | 监听对象“不需要迁移” | 迁移到 Registry/Watcher SPI |
| MBean | 42 个接口“不需要迁移” | 映射 OpenMetrics descriptor + admin operation |

对象可以没有同名公开 Rust struct，但不能没有目标对象、协议结果和验收契约。

### 10. 自动检查设计

#### 8.1 输入

- Java CodeGraph 类型清单：FQCN、kind、source path、public methods；
- Rust CodeGraph 类型清单：crate、type、source path、public methods；
- 对象账本：mapping kind、canonical target、ADR、status。

#### 8.2 规范化

```text
Java type PascalCase -> Rust type PascalCase（acronym: SQL→Sql, JDBC→Jdbc, XA→Xa, PG→Pg）
Java source FooBar.java -> Rust source foo_bar.rs
Java method lowerCamel -> Rust method snake_case
```

#### 8.3 CI 失败条件

1. Java 基线对象未登记；
2. `DIRECT` 对象没有 canonical Rust 类型或文件；
3. `MERGE/SPLIT/ADAPTER/PROTOCOL` 缺 ADR 或契约 ID；
4. 同一 Rust 文件定义多个无关迁移对象；
5. `mod.rs/lib.rs` 定义业务对象；
6. 公开方法缺中文来源注释；
7. 生产代码存在 wildcard import、`todo!()`、`unimplemented!()` 或“not yet implemented”；
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

### 11. 通过标准

命名一致性不是追求 1,719 个 Rust struct，而是实现三项 100%：

- Java 对象登记率 100%；
- `DIRECT` canonical 名称合规率 100%；
- 非一对一映射的 ADR、目标对象和语义契约绑定率 100%。

在此之前，不得使用“对象名称一致”“核心逻辑 100%”等结论。

### 14. 对象关闭条件

一个对象只有同时满足以下条件才可标 `DONE`：

- Java FQCN、源提交、Rust 类型和文件均已登记；
- 映射形态及必要 ADR 已批准；
- 字段默认值、公开方法、错误、副作用和并发语义有契约；
- Java fixture 与 Rust 测试差分通过；
- 中文 doc 注释注明 Java 来源；
- 名称检查、无 stub 检查、CodeGraph 调用链检查通过；
- 若由 enum 承载，源对象对应的 variant 不与其他对象混账。

### 归并验收记录（2026-07-29）

| 原 `docs/migration` 文档 | canonical 承接位置 |
| :--- | :--- |
| `README.md` | 本文件的三模块治理、维护规则与全局迁移治理 |
| `1、迁移路线图.md` | [迁移总路线图](./迁移总路线图.md)；模块专属阶段分别进入三个模块的 `迁移路线图.md` |
| `2、对象级对照表.md` | 三个模块各自的 `对象级对照表.md`；跨模块连接对象进入[连接抽象与驱动适配架构](./连接抽象与驱动适配架构.md) |
| `3、语义迁移对照表.md` | 三个模块各自的 `语义迁移对照表.md`；全 workspace 的 `SEM-NFR-*` 留在本文件 |
| `4、对象名称一致性检查.md` | 三个模块各自的 `对象名称一致性检查.md`；全局命名与布局门禁留在本文件 |
| `5、连接抽象与驱动适配架构.md` | [连接抽象与驱动适配架构](./连接抽象与驱动适配架构.md) |

| 检查项 | 原总账 | 归并后 | 遗漏/重复 |
| :--- | ---: | ---: | :--- |
| `SEM-*` canonical 表行 | 143 | 147（归并后新增 `SEM-FLT-017/018/019/020`） | 原 ID 0 遗漏、全部 147 个 ID 0 重复 |
| 表格首列反引号 token（对象、文件、状态、契约） | 613 | 728 | 原 token 0 遗漏 |
| Java/Rust 类型名 token（排除纯大写状态词） | 656 | 819 | 原类型名 0 遗漏 |
| `migration/` 旧目录链接 | — | 0 | 已清理 |

上表是 2026-07-29 目录归并时的历史快照；后续 C2-R27 新增
`SEM-FLT-021` 后为 148 行，C2-R28 新增 `SEM-FLT-022`，C2-R29 新增
`SEM-FLT-023`，Prepared 资源物理 setter/batch 新增 `SEM-CONN-016`；当前
在 C2-R42 新增 `SEM-FLT-024` 后为 153 个唯一 ID、0 重复。
在 C2-R43 新增 `SEM-FLT-025` 后为 154 个唯一 ID、0 重复。

2026-07-29 C2-R37 在删除状态下再次从 Git `HEAD:docs/migration` 读取旧账本：
旧 143 个唯一 `SEM-*` 全部包含于当前 152 个 canonical 行，旧 ID 遗漏 0，
canonical 重复 0。旧对象文档的 499 个反引号 token 中，当前对象/架构账本只
未原样保留 4 个非对象占位符：测试命令、两个 `...` 示例路径和
`mod.rs/lib.rs` 合写路径；Java/Rust 对象条目遗漏 0。复核后
`docs/migration` 目录保持不存在。

2026-07-29 C2-R42 再次从 Git `HEAD:docs/migration` 读取删除前基线：
旧 143 个唯一 `SEM-*` 全部存在于当前 153 个唯一 ID，遗漏 0。三个模块语义表
中的 `CORE/ADMIN/WRAP/SEM` 表行共 217 行、217 个唯一 ID、重复 0。对象级与
名称一致性旧账本的反引号 token 精确集合差只剩命令、路径通配符、状态枚举、
审计输出及 `todo!/unimplemented!` 红线示例等 15 个非对象占位符；类型名启发式
集合差只剩 `FooBar.java` 示例以及已归入完整对象名的 `MySql`/`Xa` 词根，不存在
Java/Rust 对象条目遗漏。故不恢复 `docs/migration`。

2026-07-29 C2-R43：三个模块语义表新增 `CORE-FLT-008` 与
`SEM-FLT-025` 后为 219 行、219 个唯一 ID、重复 0；workspace `SEM-*` 为
154 个唯一 ID，删除前 143 个旧 ID 仍然遗漏 0。

2026-07-29 C2-R44：新增 `CORE-FLT-009` 与 `SEM-FLT-026` 后为 221 行、
221 个唯一 ID、重复 0；workspace `SEM-*` 为 155 个唯一 ID，删除前 143 个
旧 ID 仍然遗漏 0。静态测试审计发现 Java core 475 个测试方法、Rust druid
502 个测试函数及 144 个需人工复核信号；这些数量只用于建立测试账本，不作为
语义等价证明。

2026-07-29 C2-R45：新增 `CORE-FLT-010` 与 `SEM-FLT-027` 后为 223 行、
223 个唯一 ID、重复 0；workspace `SEM-*` 为 156 个唯一 ID，删除前 143 个
旧 ID 仍然遗漏 0。静态测试审计为 Java core 475 个测试方法、Rust druid
504 个测试函数及 144 个需人工复核信号；ResultSet stream/resource 26 重载
由物理探针、资源共享生命周期及真实 Toasty SQLite 流读取建立三账本证据，
不把该数量审计或单数据库主机测试解释为全语义等价。

2026-07-29 C2-R46：新增 `CORE-FLT-011` 与 `SEM-FLT-028` 后为 225 行、
225 个唯一 ID、重复 0；workspace `SEM-*` 为 157 个唯一 ID，删除前 143 个
旧 ID 仍然遗漏 0。静态测试审计为 Java core 475 个测试方法、Rust druid
506 个测试函数及 144 个需人工复核信号；navigation/property 26 调用由精确
物理穿透、全方法短路、可失败 isClosed 分层及真实 SQLite 游标状态机建立证据。

2026-07-29 C2-R47：新增 `CORE-FLT-012` 与 `SEM-FLT-029` 后为 227 行、
227 个唯一 ID、重复 0；workspace `SEM-*` 为 158 个唯一 ID，删除前 143 个
旧 ID 仍然遗漏 0。静态测试审计为 Java core 475 个测试方法、Rust druid
507 个测试函数及 144 个需人工复核信号；NString 两重载已从 getString 折叠中
拆出，并由精确物理方法、短路错误与真实 SQLite Unicode 读取建立证据。

统计方法是将 Git 基线中的 6 份 `docs/migration` 文档拼接为旧集合，将
`docs/README.md`、两份跨模块文档和三个模块目录拼接为新集合；分别对
`SEM-[A-Z]+-[0-9]{3}`、Markdown 表格首列反引号 token、含小写字符的
PascalCase/FQCN 类型 token 去重后做集合差。该检查用于证明旧总账条目有
新归属，不等同于
Java 1,719 个生产对象的迁移完成率，也不改变任何 `PARTIAL/TODO` 状态。
<!-- GLOBAL_GOVERNANCE_MERGE_END -->
