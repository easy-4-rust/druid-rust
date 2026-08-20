# 对象级与语义迁移对照聚合规格

> 日期：2026-08-12  来源：聚合 3 模块 × (对象级对照表 + 语义迁移对照表)

## druid 模块

### 对象级对照表

> Java baseline：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`
> Rust baseline：`2c7bf3bea6f9e94f4583a85794df519c4435a425` + 当前 dirty migration worktree
> 最后静态核对：2026-07-30；文档状态：`PARTIAL`
>
> 本表冻结 Java `/core` 的 1,644 个对象分母。当前先列出主干对象与对象族；
> 每个对象未达到完整方法契约前只能标 `PARTIAL`。
>
> 这里的 1,644 是"来源审计分母"，不是要求 Rust 生成 1,644 个同名类型。
> JVM 专属对象必须入账后标为 `NOT_APPLICABLE/EXCLUDED`，并从 Rust
> "应实现分母"中排除。典型排除项包括 SLF4J、Log4j、Log4j2、Commons
> Logging、Logback、JMX MBean 与 ClassLoader 桥接对象。

#### 状态

| 状态 | 含义 |
| :--- | :--- |
| `DONE` | 当前源码存在，完整方法/字段及 Java oracle、真实依赖证据闭合 |
| `PARTIAL` | 当前源码存在，但字段、路径、分支、驱动或测试不完整 |
| `BROKEN` | 当前源码存在，但已确认关键路径错误 |
| `MISSING` | 尚无可运行的真实 Rust 落点 |
| `RENAME` | 有近似实现，但 canonical 对象或文件名不一致 |
| `ADAPTER` | Java 平台能力由 Rust 生态 Adapter 承载，仍需 Druid 契约验收 |
| `PROTOCOL` | Java 容器或平台协议由 Rust 协议实现，仍需逐项验证可观察语义 |
| `RUST_ONLY` | Rust 内部新增对象，必须注明承载的 Java 语义 |
| `MERGE` | 多个 Java 对象合并承载，必须逐个登记源对象和语义落点 |
| `SPLIT` | Java 对象拆到多个 Rust 组件，canonical facade 必须存在 |
| `NOT_APPLICABLE/EXCLUDED` | JVM/Java 平台专属对象，仅保留来源审计记录；不生成 Rust 类型，也不进入应实现分母 |

状态只表达当前证据，不表达计划。对象同名、文件存在或浅层单元测试通过，都
不能单独得到 `DONE`。

##### 映射规则

1. `DIRECT` 对象保留 Java 语义名称：类型使用 PascalCase，文件使用 snake_case。
2. `MERGE` 必须逐个列出所有源对象，不能用"其他对象"替代对象账本。
3. `SPLIT` 必须指定 canonical facade，避免调用方依赖内部拼装。
4. `ADAPTER/PROTOCOL` 必须列出外部类型、兼容层和验收契约，不能只写 crate 名。
5. 一个 `.rs` 文件只定义一个迁移对象；Builder 或内部类型可与主对象同文件。
6. `mod.rs`、`lib.rs` 只做模块声明和重导出，不堆放迁移对象。
7. Java 日志实现类不做 `DIRECT/MERGE`：只迁移事件时机、级别、字段、脱敏和
   开关到 `tracing`；不得创建 SLF4J/Log4j 风格 Rust 类型或依赖。

#### 模块落点

| Java 域 | Java 数量 | Rust 落点 | 当前 |
| :--- | ---: | :--- | :--- |
| `sql` | 1,268 | `druid/src/sql` | PARTIAL |
| `support` | 105 | `druid`；管理协议部分归 `druid-admin` | PARTIAL |
| `pool` | 72 | `druid/src/core` + `druid/src/pool` | PARTIAL |
| `wall` | 46 | `druid/src/sql/wall` | PARTIAL |
| `proxy` | 36 | `druid/src/core/proxy` | PARTIAL |
| `util` | 35 | `druid/src/util`，按对象迁移 | PARTIAL |
| `filter` | 29 | `druid/src/core/filter` + `druid/src/stats` + `druid/src/sql/wall` | PARTIAL |
| `stat` | 24 | `druid/src/stats` | PARTIAL |
| `mock` | 22 | test fixtures，不计生产完成 | PARTIAL |
| core 根对象 | 7 | `druid` 根门面 + `druid/src/core` | PARTIAL |
| Java/JDBC 数据库平台 | 不计入 1,644 | `druid/src/toasty` 内置默认实现；`druid-wrapper` 扩展 | ADAPTER |

#### Pool/连接主对象

| Java 对象 | 映射 | Rust 对象/文件 | 当前职责与缺口 |
| :--- | :--- | :--- | :--- |
| `DruidDataSource` | SPLIT | `druid_data_source.rs` / `DruidDataSource` + `druid_pool.rs` / `DruidPool` engine | canonical facade、配置工厂、维护与管理协议已存在；fatal 阈值/借用门禁/即时 discard/shrink 重验生产链已接，完整 Java 配置、XA、驱动矩阵与统一差分仍为 PARTIAL |
| `DruidAbstractDataSource` | SPLIT | `config.rs` / `PoolConfig`、`PoolInnerConfig`、`DruidPoolBuilder` | 池主配置、默认值、属性绑定、边界校验与运行读取已迁入；全字段和动态 setter 仍为 PARTIAL |
| `DruidConnectionHolder` | DIRECT | `druid_connection_holder.rs` | 所有权/reset/schema/PS cache、线程安全 ConnectionEventListener/StatementEventListener 列表、普通/Prepared/Callable statement trace 及 PhysicalConnectionInfo 会话/全局变量持有已接；MySQL `show variables/show global variables` 实际采集已进入创建链，真实 MySQL 与差分待验 |
| `DruidPooledConnection` | DIRECT | `druid_pooled_connection.rs` | recycle、prepared/callable 子集、三个 createStatement 重载、Wrapper 解包、结构化 SQLWarning/Filter around-chain、ConnectionEventListener 添加/移除/关闭一次通知/错误通知、所有 Statement 创建/显式关闭/回收 trace，以及 PhysicalConnection/SQL Filter 错误的 sorter→数据源 fatal 状态→即时 holder/物理连接 discard 主链已迁移；完整统计与统一差分仍缺失 |
| `PreparedStatementPool` | DIRECT | `prepared_statement_pool.rs` | LRU/inUse/share/clear；Oracle implicit cache 缺失 |
| `PreparedStatementHolder` | DIRECT | `prepared_statement_holder.rs` | key/计数/peak；驱动属性矩阵缺失 |
| `DruidPooledPreparedStatement` | DIRECT | `druid_pooled_prepared_statement.rs` | execute/cache/lease、self/PreparedStatement/raw concrete unwrap、clearParameters/clearBatch fatal-discard/cache 淘汰、48 个 JDBC setter 重载与持久参数槽、绑定 query/update/generic execute、有序描述符 batch、顺序多结果/generated keys/getter sorter/ResultSet trace/继承 SQLWarning 与 Statement 属性已验证；五个缓存敏感属性按 Java 顺序初始化、更新并在回收前恢复，首错停止且禁止异常句柄重入缓存；canonical `executeQuery` 直接持有物理 ResultSet，共享生命周期与 handle 使 ResultSet 恢复同一 Prepared 身份；Toasty/SQLx SQLite 资源主链及 RBDC 严格 SPI 已接，原生多数据库 generic execute/ResultSet 仍缺 |
| `PreparedInputParameter` | RUST_SUPPORT | `prepared_input_parameter.rs` | 一个 Rust 对象承载 Java PreparedStatement 各 setter 的输入联合类型；保留 null/typeName、Calendar、Object 元数据、stream/reader 长度和 JDBC 资源身份，并作为 Filter/物理参数批次协议；`RustValue` 仅标识原 Rust 显式 batch 扩展，不冒充 Java setter；不是 Java 新对象计数项 |
| `ToastyPreparedStatement` | PLATFORM_ADAPTER | `toasty/toasty_prepared_statement.rs` | Toasty raw SQL 缺少独立 PreparedStatement 槽位时的物理句柄；setter 阶段物化资源并保存参数，负长度/提前 EOF 当场返回，addBatch 保存值快照，execute 不二次读取；继承 Statement 属性委托真实句柄状态 |
| `DruidPooledCallableStatement` | DIRECT/ADAPTER | `druid_pooled_callable_statement.rs` | Java 本类构造器之外 115 个公开声明均有 Rust 同语义入口；继承的 execute/result-set/generated-keys/more-results 路径复用 Prepared 基类，并由 `DruidPooledCallableStatementHandle` 保留 callable 动态身份、Prepared 继承能力和关闭级联。支持存储过程的真实 driver、错误和结果资源证据仍未闭合 |
| `DruidPooledStatement` | DIRECT | `druid_pooled_statement.rs` | PARTIAL：独立对象、同租约校验、query/update、JDBC `int[]` batch/部分失败、四种 `execute` 入口、结果类型判定、顺序多结果、`getResultSet/getUpdateCount/getMoreResults/getGeneratedKeys`、结构化 SQLWarning、属性、关闭、Wrapper 与 ResultSet trace 已验证；SQLx/RBDC Connection warning Adapter 已闭合，原生多结果及其余 Filter statement 事件仍缺失 |
| `DruidPooledResultSet` | DIRECT/ADAPTER | `druid_pooled_result_set.rs` + `PhysicalResultSet`/`RowSetResultSet` + `ResultSetStatement` | PARTIAL：既有完整 getter/update/metadata/Wrapper 基础上，next/close/warning、9 族标量 18 getter、BigDecimal/Date/Time/Timestamp 16 个强类型重载、plain/map/typed object 六重载、13 族 stream/resource 26 重载、navigation/property 26 调用、NString 两重载、getMetaData、getStatement、七个 row-mutation、14 族 index/label 共 28 个基础列 update setter、`updateObject` plain/scaleOrLength × index/label 四重载、七类资源对象 index/label 14 个 setter、Blob InputStream 与 Clob/NClob Reader 的 12 个 LOB setter、ASCII/Binary/Character/NCharacter 的 22 个 stream setter，以及 `updateNString` 的 index/label 两个 setter 均同步进入 Filter around-chain；`ResultSetStatement` 保留普通/Prepared/Callable 三种动态身份与同一逻辑生命周期；row-mutation、基础/object/resource/LOB/stream/NString setter 已验证默认穿透、全方法短路、参数身份、共享游标、错误分类及真实 SQLite capability error，vendor custom object 可下转回原类型，资源链内拥有型 Clone 保持共享句柄；其余精确参数、共享资源与 metadata 平台身份由探针和真实 Toasty SQLite 验证；`is_closed_with_connection` 保留 Java 可失败入口；StatFilter、typed custom object 单次物理读取和最小 SPI 错误优先级已闭合；185 个 ResultSet 调用保留精确委托，ResultSet update setter Filter 子域已闭合；Statement 与 PreparedStatement canonical query 不再经 `Vec<Row>` 重建，SQLx 真实 SQLite 与 RBDC SPI driver label 可贯穿 metadata/findColumn/label getter；getString/getBytes/stream 读取量及直接/间接 LOB open 已接 SQL 统计；Toasty eager/raw SQL alias 缺失、RBDC 空结果 descriptor、嵌套 ResultSet/Clob object 自动代理、streaming、driver custom conversion 与多数据库仍待补 |
| `ConnectionEventListener` | ADAPTER | `connection_event_listener.rs` + holder/pooled connection 生产接线 | IMPLEMENTED_UNVERIFIED：Rust 最小 SPI 已承载关闭和错误通知；listener 列表、对象身份移除、关闭一次通知及回收清理已接，最终统一测试待做 |
| `StatementEventListener` | PLATFORM_ADAPTER | `statement_event_listener.rs` + holder/pooled connection 生命周期 | IMPLEMENTED_UNVERIFIED：Rust 最小 SPI 保留两个 JDBC 回调签名；按 Java Druid 现状仅迁移 listener 增删与 reset/recycle 清理，不擅自发布 Java 源码未发布的事件；最终统一测试待做 |
| `ExceptionSorter` | DIRECT | `exception_sorter.rs` + 11 个独立 vendor/abstract 文件 | `SqlException` 保留 code/SQLState/recoverable/class/message/cause/`instanceof` 类型链；Java 全部 11 个 vendor/abstract 对象均已独立迁移，池化主链仍按独立账目追踪 |
| `ValidConnectionChecker` | DIRECT | `valid_connection_checker.rs` | 真实驱动超时矩阵不完整 |

#### Proxy/Filter 对象族

| Java 对象/对象族 | Rust 落点 | 当前 |
| :--- | :--- | :--- |
| `Filter` / `FilterAdapter` / `FilterEventAdapter` / `FilterManager` / `FilterChain` / `FilterChainImpl` | `filter.rs` + `filter_adapter.rs` + `filter_event_adapter.rs` + `filter_manager.rs` + `filter_chain.rs` + `filter_chain_impl.rs` + `result_set_filter*.rs` | PARTIAL：canonical `FilterChainImpl` 已存在并由 `FilterChain` 接口兼容名直接指向，承载统一实例注册、生命周期、dataSource get/release、物理 connect、SQL/batch/连接事件、warning、Connection LOB 创建有位置链及已迁移 ResultSet 精确 around-chain；DataSource get 可修改 maxWait/短路，release 可包围/短路唯一 holder 回收，`createBlob/createClob/createNClob` 可替换返回值/短路/报错，ResultSet update setter 子域已闭合。Java 498 个 public/protected 方法中，完整 Connection/Statement/Prepared/Callable/LOB/metadata hook、具体 proxy 身份和 nested downstream-before 展开仍未闭合，不能因 canonical 文件存在而标 DONE |
| `ConnectionProxy/Impl` | `DruidPooledConnection` + `ProxyAttributes` + `TransactionInfo` | MERGED/PARTIAL/IMPLEMENTED_UNVERIFIED：连接 ID、raw/unwrap、惰性 attributes、物理创建 epoch、显式关闭次数、最近成功验证时间、活动事务信息、连接逻辑驱动 properties、`createBlob/createClob/createNClob` 及 before/commit/rollback/close Filter 事件身份已进入 canonical 池化连接；各 Adapter 对 vendor properties/LOB 的真实应用和统一差分仍缺 |
| `StatementProxy/Impl` | `DruidPooledStatement` + `ExecContext` | MERGED/PARTIAL/IMPLEMENTED_UNVERIFIED：数据源级 20000 起始 ID、attributes、raw/unwrap、最近 SQL/耗时/update/result/batch 状态及 Filter 执行/创建/关闭身份已接；ExecContext 拥有可改写 SQL，普通 Statement 的 Filter 输出已进入物理驱动并回写 lastSql，addBatch 在保存前逐条改写；完整 Java getter 仍开放 |
| `PreparedStatementProxy/Impl` | `DruidPooledPreparedStatement` + `PreparedInputParameter` | MERGED/PARTIAL/IMPLEMENTED_UNVERIFIED：继承同一 Statement ID/attributes/执行状态，参数槽、setter 描述符、batch 快照、JdbcParameter 合同及 0-based Proxy 参数视图已接；剩余 Filter hook 待补 |
| `CallableStatementProxy/Impl` | `DruidPooledCallableStatement` | MERGED/PARTIAL/IMPLEMENTED_UNVERIFIED：继承 Prepared/Statement 身份、attributes、参数与结果状态；真实 callable driver 及剩余 hook 待补 |
| `ResultSetProxy/Impl` | `DruidPooledResultSet` + `ResultSetFilterContext` + `ResultSetOpenContext` | MERGED/PARTIAL/IMPLEMENTED_UNVERIFIED：数据源级 50000 起始 ID、attributes、raw/unwrap、SQL、Statement 身份、construct elapsed、游标/fetch/close/read/open、logic↔physical column map、nullable hidden columns 与三层 Filter ID 已接；Wall tenant metadata 已自动生产并绑定到每个 ResultSet，完整日志列值和统一差分待补 |
| `ClobProxy` / `ClobProxyImpl` / `NClobProxy` / `NClobProxyImpl` | 四个 canonical Druid 对象已独立落位；`JdbcClob/JdbcNClob` 仍只承载平台 raw 资源 | PARTIAL/IMPLEMENTED_UNVERIFIED：13 个 Clob 操作从 position=0 进入 BeforeFilter around-chain，末端才调用同一 raw Clob；ResultSet 提供 index/label Proxy 包装，EncodingConvertFilter 的 string/Reader Clob 分支已进入链；Connection#createClob/createNClob 已走独立有位置链并返回对应 Druid Proxy，真实 LOB 驱动待补 |
| `BlobProxy` / `BlobProxyImpl` | Java 1.2.28 baseline 无此对象 | NOT_APPLICABLE/EXCLUDED | 不因旧表误写制造 Rust 对象；`JdbcBlob/PhysicalBlob` 只映射 `java.sql.Blob` |
| `StatFilter` | `druid::stats::StatFilter` | PARTIAL：SQL query/update/generic execute、普通与 PreparedStatement batch、活动事务 commit/rollback、ResultSet fetch/hold/read/stream、数据源/SQL 双层 LOB open、pool get/release 及真实物理 connect/close around-chain 已接生产链；beforeConnect/connectError/afterConnected/Entry 注册与 close count/Entry 移除只由启用的 StatFilter 产生，不由池无条件伪造；完整字段及统一差分待补 |
| `WallFilter` | `druid::sql::WallFilter` | PARTIAL/IMPLEMENTED_UNVERIFIED：独立对象已实现 SQL before/after、拒绝错误、SELECT/UPDATE/INSERT tenant AST 改写、普通 Statement 物理透传与 addBatch 保存前改写、Prepared/Callable prepare 前改写、raw metadata 隐藏列映射与 tenant next 回调、Connection metadata Wall 开关及 privileged 绕过，并由内置 FilterManager 工厂装配；完整 Java hook、配置项与多方言规则仍开放 |
| `LogFilter` | tracing Adapter | PARTIAL：canonical `LogFilter` 只迁移 Druid 的管理配置、开关和事件语义；`log` 仅为 Druid Filter 配置短名，运行时后端统一为 `tracing`；SQL/batch、Statement 创建/关闭、connection commit/rollback、pool get/release、真实物理 connect/close 与 ResultSet open/next/close/error 已输出相应 Proxy ID，物理 connect/close 均遵守嵌套链时机；Java Log4j/SLF4J/Commons Logging 类和 MBean明确 `NOT_APPLICABLE`，不以对象数量为目标；完整 ResultSet 列值、动态 tracing level 和 executable SQL 格式仍缺 |
| `ConfigFilter` / `ConfigTools` | `core/filter/config/{config_filter,config_tools}.rs` | PARTIAL/IMPLEMENTED_UNVERIFIED：canonical 对象已保留配置优先级、properties/XML、本地/classpath/HTTP(S)、旧 RSA 密文、错误前缀和工厂前置装配；Java/Rust 密文向量、HTTP/XML 与真实数据源统一验证待最终阶段 |

#### SQL/Wall/Stat 对象族

| Java 对象/对象族 | Rust 落点 | 当前 |
| :--- | :--- | :--- |
| `SQLObject` / statement / expression AST | `druid::sql` compatibility AST | MISSING/PARTIAL |
| parser/lexer/dialect parsers | sqlparser-rs + Druid dialect extension | PARTIAL |
| Visitor/OutputVisitor | compatibility visitors | MISSING |
| `SQLUtils` / `JdbcUtils` / `DbType` | canonical facade objects | PARTIAL：`DbType` 严格名称、两别名、mask/hash/equals/PostgreSQL-style 已落；`JdbcUtils` URL/driver 子集已有；`SQLUtils` 与 JdbcUtils 全方法仍缺 |
| `WallConfig` | `wall_config.rs` | PARTIAL：tenantColumn、tenantTablePattern、doPrivilegedAllow、运行期线程安全 TenantCallBack 与 update-check 配置已接；完整 Java 字段和规则生效矩阵仍开放 |
| `WallConfig.TenantCallBack` / `TenantCallBack.StatementType` | `tenant_call_back.rs` / `TenantCallBack` + `TenantStatementType` | DIRECT/IMPLEMENTED_UNVERIFIED：四个回调方法、四种语句类型与 ResultSet tenant 值回调已接；SQL AST tenant 注入和统一差分待补 |
| `WallProvider` | `sql/wall_provider.rs` provider/cache facade | PARTIAL：规则检查、缓存、白/黑名单、统计 API、同步 privileged 嵌套恢复与 async task-local scope 已有；完整方言 provider/visitor、tenant/mustParameterized 仍缺 |
| violation 对象族 | `WallViolation` variants | PARTIAL |
| `JdbcSqlStat` | `stats/jdbc_sql_stat.rs` | PARTIAL/IMPLEMENTED_UNVERIFIED：成功/错误、最近错误、running/concurrent、事务、执行/hold 时间与 histogram、batch、update/fetch、lastSlowParameters、读取量/流/LOB 打开数已接生产链；执行开始/最大耗时发生时点已接，管理 ID 独立递增、HASH 精确使用 Java UTF-16 FNV-1a；JVM stack trace、LOB 内容读取统计及统一差分未闭合 |
| `JdbcDataSourceStat` | `stats/jdbc_data_source_stat.rs` | PARTIAL：canonical 对象聚合 SQL merger、Connection/Statement/ResultSet、慢 SQL 与独立 resetStatEnable gate；`StatsCollector` 仅为兼容 type alias；分层完整字段和统一差分未闭合 |
| `DruidStatManagerFacade` / `DruidStatService` | `stats/druid_stat_manager_facade.rs` + `druid_stat_service.rs` | PARTIAL/IMPLEMENTED_UNVERIFIED：datasource/sql/wall/connectionInfo/active trace/reset JSON 协议已接；resetAll 按 Jdbc manager→datasource manager→facade count 分层执行，log-and-reset 独立发布区间快照；basic 输出真实 StartTime/Rust MSRV/target，并从已注册数据源枚举去重 driver label，Java runtime 兼容键保持 null；SQL zero/non-running 过滤及 Wall 递归聚合已接；未实例化 Adapter 的全局 Drivers、Java web/spring 分支和逐字段 golden 未闭合 |

#### Java 平台对象分账

| Java 平台对象 | Rust 落点 | 规则 |
| :--- | :--- | :--- |
| `java.sql.Connection` | `PhysicalConnection` | 内部最小 SPI |
| JDBC driver factory/connection | `ToastyConnectionFactory` + `ToastyConnectionAdapter` | `druid` 内置标准实现；直接使用 Toasty Driver SPI，不使用 `toasty::Db` pool |
| `java.sql.PreparedStatement` | `PhysicalPreparedStatement` | 真实 driver handle |
| `java.sql.CallableStatement` | `PhysicalCallableStatement` + `CallableInputParameter`/`JdbcObject`/`CallableParameter`/`CallableOutParameter` | setter 身份及 sqlType/typeName/scale 必须保留；`CallableOutputValue` 仅为 `JdbcObject` 兼容 alias；不支持时明确报错 |
| `java.lang.Object`（JDBC 值域） | `JdbcObject` + `JdbcOpaqueObject`/`PhysicalJdbcOpaqueObject` | 标准标量、资源和 vendor custom 对象统一平台表示；自定义对象保留引用身份、类名与受控 downcast |
| `java.lang.Class<T>`（JDBC typed getter） | `JdbcTargetType` | 保留 typed `getObject` 的标准目标和 vendor 类名；`CallableTargetType` 仅为兼容 alias |
| `Map<String, Class<?>>` | `JdbcTypeMap` | ResultSet/Callable/Array/Ref 共用；`CallableTypeMap` 仅为兼容 alias |
| `java.sql.ResultSetMetaData` | `ResultSetMetaData` + `PhysicalResultSetMetaData` + `ResultSetColumnMeta`/`ResultSetColumnType`/`ResultSetNullability` | 双后端句柄：eager descriptor 与 physical driver metadata；Java 标准 getter、逐调用错误时机、self/SPI/concrete Wrapper 身份、三态可空性、Types/class/origin/shape/读写标志完整表达；真实字段只由 driver descriptor 提供，不猜测 |
| `java.sql.ResultSet#getStatement()` 返回对象 | `ResultSetStatement` | ADAPTER：Rust 拥有型枚举保留普通、Prepared、Callable 三种动态身份；各分支 Clone 共享同一逻辑内核、关闭状态和缓存所有权，不是字段快照，也不把三种对象错误合并为单一最小接口 |
| `java.sql.Blob` | `JdbcBlob` + `PhysicalBlob` | 完整 11 方法资源 SPI；对象身份比较，不隐式读取内容；驱动负责实际存储和错误 |
| `java.sql.Clob` | `JdbcClob` + `PhysicalClob` | 完整 13 方法资源 SPI；位置/长度为 Java 有符号类型；禁止静默物化为 Rust `String` |
| `java.sql.NClob` | `JdbcNClob` + `PhysicalNClob` | 保留 `NClob extends Clob` 类型身份和完整继承操作 |
| `java.lang.String` | `RdbcString` | UTF-16 code unit 值对象；保留未配对 surrogate，严格转 UTF-8 时显式报错 |
| `java.io.Reader` / `Writer` | `RdbcReader` / `RdbcWriter` | Clone 共享游标/关闭状态；字符以 UTF-16 code unit 传递；Callable setter 不预读 |
| `java.io.InputStream` / `OutputStream` | `RdbcInputStream` / `RdbcOutputStream` | Clone 共享游标与关闭状态；Callable setter 不得提前物化 |
| `java.util.Calendar` | `JdbcCalendar` + `JdbcCalendarArgument`（`CallableCalendar*` 为兼容别名） | 保留时区标识，并区分无 Calendar 重载、显式 null 与实际 Calendar；Callable/ResultSet 共用 |
| `java.math.BigDecimal`、`java.sql.Date/Time/Timestamp` | `bigdecimal::BigDecimal` + `chrono::NaiveDate/NaiveTime/NaiveDateTime` | 使用生态强类型；禁止压缩成 String/Bytes |
| `java.sql.ResultSet` | `PhysicalResultSet` + `RowSetResultSet` + `ResultSetUpdate` | 当前内置 SQLite 为可滚动只读 eager Adapter；1-based 索引、NULL、游标、fetch/type、基础/流/资源/Decimal/日期时间 getter、全部标量与流 update 重载身份、Map/typed getObject SPI 与完整 metadata getter 对象契约已落；通用对象及 String/Boolean/Long/Int/Short/Byte/Double/Float/Bytes 的 index/label 物理 SPI 可被 driver 独立覆盖，默认 `Value` Adapter 的 NULL/转换/错误合同已测；最小 SPI 的精确 operation、错误优先级及 custom typed object 单次读取已锁定；平台已支持 physical metadata，但 Toasty 尚未供给真实 label/origin descriptor；streaming、真实 custom conversion 与多数据库实现待补 |
| `java.sql.SQLException` | `DruidError` + 后续结构化物理错误 | 保留错误分类 |
| `java.sql.BatchUpdateException` | `DruidError::BatchUpdateException { update_counts, cause }` | 保留 `int[]` 部分更新计数、`-2/-3` 表达空间和原始 SQL cause；fatal sorter 递归检查 cause |

##### 内置与扩展对象分账

| 层级 | 对象 | 当前模块/目录 | 归并来源（历史） | 是否内置 | 所有权 |
| :--- | :--- | :--- | :--- | :---: | :--- |
| Druid pool | `DruidPooledConnection` | `druid/src/core` | `druid-core` | 是 | 对外唯一池化连接 |
| 稳定 SPI | `PhysicalConnection` | `druid/src/core` | `druid-core` | 是 | 只表示一条 raw connection |
| 标准 factory | `ToastyConnectionFactory` | `druid/src/toasty` | `druid-toasty` | 是 | 共享无状态 Driver |
| 标准 adapter | `ToastyConnectionAdapter` | `druid/src/toasty` | `druid-toasty` | 是 | 独占一条 Toasty raw connection |
| direct 扩展 | `SqlxConnectionAdapter`/`RbdcConnectionAdapter` | `druid-wrapper/src/{sqlx,rbdc}` | `druid-sqlx`/`druid-rbdc` | 否 | 各独占一条 raw connection |
| external pool | `SqlxBb8Pool`/`SqlxDeadpoolPool` | `druid-wrapper/src/sqlx/{bb8,deadpool}` | 原两个 SQLx pool crate | 否 | 直接实现 `Pool`，不得进入 native idle queue |

本文件已经承接原仓库级对象总账中属于 Java core 的全部明细；本模块完成率只以
1,644 个 Java core 对象逐项关闭为准。

#### 五 Crate 目标归属（ADR-CRATE-001）

> 以下为已批准的五 Crate 目标归属。当前源码仍为三 Crate，实现状态保持当前事实。

| 目标 Crate | 归属对象 |
| :--- | :--- |
| `druid-core` | RDBC/JDBC 类型、Pool、Filter、SQL、Wall、Dynamic、统计原始状态和 typed snapshot |
| `druid-wrapper` | Toasty/SQLx/RBDC/DuckDB/libSQL/HTTP SQL/JDBC Agent、vendor checker/sorter、driver tooling |
| `druid-metrics` | registry、sampler、timeline、Prometheus model、gRPC protocol/runtime |
| `druid-admin` | ingest repository、REST、认证、兼容静态 UI、独立 binary |
| `druid`（facade） | stable re-exports and optional features only |

Toasty 目标归属 Wrapper（当前态：Toasty 属于 `druid` 内置默认实现；目标态：
Toasty 收敛到 `druid-wrapper`）。

`StatFilter` 和统计状态留在 Core；全局 registry/facade/exporter 移入 Metrics；
HTTP/REST service 移入 Admin。

---

### 语义迁移对照表

> Java baseline：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`
> Rust baseline：`2c7bf3bea6f9e94f4583a85794df519c4435a425` + 当前 dirty migration worktree
> 最后静态核对：2026-07-30；文档状态：`PARTIAL`（`V0_STATIC`，尚未统一验收）

#### 状态规则

| 状态 | 定义 |
| :--- | :--- |
| `PASS` | Java oracle 与 Rust 真实依赖路径均通过 |
| `PARTIAL` | 子集已通过，但方法/配置/驱动矩阵不完整 |
| `FAIL` | 已有实现与 Java 可观察结果不同 |
| `TODO` | 尚无实现或证据 |
| `NOT_APPLICABLE/EXCLUDED` | JVM/Java 平台实现细节；只做来源审计，不生成 Rust 对象、不进入实现完成率 |

#### 连接池

| ID | Java 语义 | Rust 当前 | 状态/缺口 |
| :--- | :--- | :--- | :--- |
| CORE-POOL-001 | maxActive 原子容量 | 原子预留与压力测试 | PARTIAL |
| CORE-POOL-002 | maxWait、公平等待、超时 | `-1` 无限等待、0/正数截止时间、取消安全 wait 计数、`maxWaitThreadCount` 已接；公平锁切换未闭合 | PARTIAL |
| CORE-POOL-003 | initialSize/minIdle/fill | init 预建、asyncInit 受监管后台目标、initExceptionThrow=true 的"已初始化后返回首错"、false 的固定 3s 重试、显式 `fill()` 到 maxActive、`fill(toCount)` 截断 maxActive、后台 minIdle fill、keepAlive 失败补池和配置边界已接；enable=false 只禁止借用/回收，不阻止 init/fill | PARTIAL：待初始化错误/异步任务统一差分 |
| CORE-POOL-004 | testOnBorrow/testOnReturn/testWhileIdle | 三开关、统一 ValidConnectionChecker、borrow/return/idle 主链已接 | PARTIAL：待多驱动与差分 |
| CORE-POOL-005 | rollback/reset/schema restore | 主顺序与 SQLite 契约 | PARTIAL |
| CORE-POOL-006 | shrink/phyTimeout/phyMaxUseCount/keepAlive | Java `checkTime` 分支、顺序保活、失败销毁、minIdle 回填、fatal 前连接强制重验及取消安全批次已接 | PARTIAL/IMPLEMENTED_UNVERIFIED：fatal 与普通 shrink 组合差分待补 |
| CORE-POOL-007 | removeAbandoned/async close | active lease 追踪、运行中跳过、超时失效、backtrace/脱敏日志已接；2026-08-13 补 3 差分测试（超时回收/running 守卫经阻塞 exec 验证/开关门，对照 DruidDataSource#removeAbandoned + TestAbondon fixture）；Rust Drop/异步关闭映射待统一验收 | PARTIAL（removeAbandoned 子域 DONE） |
| CORE-POOL-008 | exactly-once recycle/discard | native/bb8/deadpool 已测 | PASS |
| CORE-POOL-009 | shutdown 排空任务和租约 | close 先封池、取消安全排空 idle、等待维护任务；活跃租约的强制终止仍开放 | PARTIAL |

#### Connection/Statement

| ID | Java 语义 | Rust 当前 | 状态/缺口 |
| :--- | :--- | :--- | :--- |
| CORE-CONN-001 | exec/query 与参数类型 | Toasty SQLite NULL/BOOL/I64/F64/DECIMAL/DATE/TIME/TIMESTAMP/TEXT/BLOB 真实绑定；声明列类型恢复强类型；raw BOOLEAN 按 SQLite storage class 返回 INTEGER | PARTIAL：PostgreSQL/MySQL/Turso 未闭合 |
| CORE-CONN-002 | transaction/savepoint | Toasty SQLite begin/commit/rollback/setAutoCommit/named savepoint 真实测试 | PARTIAL：多数据库/XA 缺失 |
| CORE-CONN-003 | PreparedStatement 六重载 key/cache | 主语义及 self/PreparedStatement/raw concrete unwrap 已测；`close_with_connection` 先按 Java 固定顺序恢复五个 Statement 属性，再处理 clearParameters/clearBatch 和 cache；首个恢复错误立即停止并增加 exceptionCount；完整 JDBC `setXxx` 重载由 `PreparedInputParameter` 保留类型、nullable、Calendar、长度与资源身份；绑定可驱动 query/update/generic execute 与有序 batch；Toasty/SQLx SQLite 及 RBDC 严格 SPI 的物理句柄在 setter 阶段物化资源，Filter 单次/批次仍收到原描述符；canonical query 直接包装 `PhysicalResultSet`，不再经 `Vec<Row>` 重建；继承属性、SQLWarning、共享 ResultSet trace 及 Prepared 动态身份已接 | PARTIAL：无连接参数 close 无法运行 ExceptionSorter；RBDC 组合实库及 SQLx Any/PostgreSQL/MySQL、Toasty PostgreSQL/MySQL/Turso generic/batch/ResultSet 矩阵缺失 |
| CORE-CONN-004 | CallableStatement 三重载/IN/OUT | Java 构造器之外 115 个公开方法均有 Rust 同语义入口；另有 5 个 Rust 生命周期/扩展方法；self/Callable/Prepared/raw concrete unwrap 已闭合 | PARTIAL：API 面已闭合，真实存储过程 driver、结果资源与 vendor 错误矩阵未闭合 |
| CORE-CONN-005 | ResultSet 生命周期/metadata | canonical `DruidPooledResultSet` + `PhysicalResultSet`/`RowSetResultSet`；只读游标、基础/流及强类型 getter、资源对象 16 getter、对象型 14 update、LOB Reader/InputStream 12 update、标量/字符/二进制流 56 update、Map/typed getObject 各 2 重载、Calendar 三态、NULL/wasNull、SQLWarning、完整 metadata 与 Wrapper；next/close/warning、九族标量 18 getter、BigDecimal/Date/Time/Timestamp 16 个强类型重载、plain/map/typed object 六重载、stream/resource 26 重载、navigation/property 26 调用、NString 两重载、getMetaData、getStatement、七个 row-mutation 无参调用、updateNull 与 13 个 typed setter 的 index/label 共 28 个基础列更新调用、updateObject 的 plain/scaleOrLength × index/label 四重载、Ref/Blob/Clob/Array/RowId/NClob/SQLXML 的 index/label 14 个资源对象 setter、Blob InputStream 与 Clob/NClob Reader 的 12 个 LOB setter、ASCII/Binary/Character/NCharacter 的 22 个 stream setter，以及 `updateNString` 的 index/label 两个 setter，均由位置化 Filter around-chain 同步转发；`ResultSetStatement` 保留普通/Prepared/Callable 三种动态身份、同一逻辑生命周期和 Filter 替换能力，标签、Calendar 三态、nullable Map、目标类型、资源与 metadata 平台身份保持可辨；`is_closed_with_connection` 保留 Java 可失败 Filter 入口，内部 `is_closed` 仅作无失败生命周期观察器；StatFilter open/close/fetch/hold、错误路由、同租约与 Statement trace 已验证；`JdbcOpaqueObject` 保留 vendor object 身份并可下转回原类型；custom typed object 精确一次物理列读取，最小 SPI 的导航/属性/update 默认方法逐项保留 operation 名称，并优先传播列索引或底层读取错误；185 个 ResultSet 调用逐方法精确委托，ResultSet update setter Filter 子域已闭合；Statement/PreparedStatement canonical query 均直接持有物理 ResultSet，SQLx 真实 SQLite（含普通 Statement 零行 query）与 RBDC SPI 的 driver label 可贯穿 metadata/findColumn/label getter，物理或 Filter 错误在同一 Java 调用点分类并计入 Statement | PARTIAL：Toasty 仍为 eager RowSet 且 raw SQL alias 不保留；RBDC 空结果 metadata、SQL 级 read length、streaming、多数据库、嵌套 ResultSet/Clob 自动代理与 vendor custom typed conversion 仍待补 |
| CORE-CONN-006 | LOB/Array/Ref/SQLXML | Blob 11 方法、Clob 13 方法、NClob 继承契约及 Array/Ref/RowId/SQLXML/URL 独立平台资源已实现；额外恢复 Druid `ClobProxy/Impl`、`NClobProxy/Impl` 四对象，13 个操作进入有位置 FilterChain，ResultSet 可产生 index/label Proxy；EncodingConvertFilter 的 Clob 字符/Reader 分支已接；`PhysicalConnection` 与 `DruidPooledConnection` 已补齐 `createBlob/createClob/createNClob`，三入口逐次从 position=0 进入 FilterChain，Clob/NClob 返回 Druid Proxy，Blob 按 Java 保持 raw 句柄 | PARTIAL/IMPLEMENTED_UNVERIFIED：真实 LOB Adapter 与 Java/SQLite capability 差分尚未闭合；Java baseline 无 BlobProxy，不制造伪对象 |
| CORE-CONN-007 | fatal error/discard | `SqlException` 可承载 code/SQLState/recoverable/class/message/cause/可赋值类型链；11 个 Java vendor/abstract sorter 对象均为独立文件；23 条 `PhysicalConnection` 错误路径统一进入 sorter；SQLx 保留公开 database fields，Toasty/RBDC 保留各自上游公开信息上限 | PARTIAL：Java vendor oracle 23/23 + Oracle 连接/语句全族 53/53、Rust sorter 10/10、Rust 全路径 1/1、真实 SQLite Toasty structured-error 与 SQLx fatal-discard 均通过；Statement 全操作、上游未公开的 vendor 字段、listener/错误统计待闭合 |
| CORE-CONN-008 | SQLite 表达式列 runtime type | `COUNT(*)` 回归已修复 | PASS |
| CORE-CONN-009 | `WrapperAdapter`/`PoolableWrapper`/池化连接与语句 unwrap | `Any/TypeId` 类型令牌 + `Unwrapped` 区分具体对象与 Connection/Prepared/Callable 接口；Java connection 34/34 + statement 28/28、Rust wrapper 3/3 + prepared/callable、真实 SQLite wrapper 3/3 | PASS（Wrapper 对象语义；metadata 与生产 Proxy 对象另行验收） |
| CORE-CONN-010 | `DruidPooledStatement` 与 `createStatement` 三重载 | 独立 `PhysicalStatement` SPI 和 canonical `DruidPooledStatement` 已承接默认/类型并发/完整保持性三入口、query/update、JDBC `int[]` batch、四种 `execute`、结果类型判定、顺序多结果、`getResultSet/getUpdateCount/getMoreResults/getGeneratedKeys`、SQLWarning、属性、关闭、跨租约拒绝和 Wrapper；Toasty 与 SQLx SQLite warning/clear 已实测，RBDC Connection warning SPI 已测 | PARTIAL：普通 Statement 主链已闭合；SQLx/RBDC 原生多结果/生成键矩阵、其余 Filter statement 事件和完整统计尚未闭合 |
| CORE-CONN-011 | `DruidPooledPreparedStatement#addBatch/clearBatch/executeBatch` | wrapper 在 add 时保存独立 `Vec<Value>` 快照；`clearParameters` 不动已入批参数；before Filter 短路保留批次，物理调用开始后成功/失败均消费；默认 SPI 顺序复用同一 PreparedStatement 并保留部分计数；真实 Toasty/SQLx SQLite 与 RBDC 严格 SPI 已测 | PARTIAL：Druid/Filter/Stat 主语义已闭合；RBDC 组合实库、PostgreSQL/MySQL 原生 batch、`-2/-3` 与驱动专属异常矩阵待补 |
| CORE-CONN-012 | `DruidPooledPreparedStatement#execute/getResultSet/getUpdateCount/getMoreResults/getGeneratedKeys` | `PhysicalConnection::execute_prepared` 保留参数快照和 prepare 重载键；组合 `DruidPooledStatement` 基类状态承接结果类型、顺序多结果、generated keys、继承 SQLWarning、wrapper 关闭与 ResultSet trace；`DruidPooledPreparedStatementHandle` 让 query/current/generated-keys ResultSet 恢复同一 Prepared 身份，`DruidPooledCallableStatementHandle` 再保留 Java callable 子类型身份；两者共享 key、物理 unwrap 和关闭级联；物理 getter 错误按发生时点进入 sorter | PARTIAL：Druid/Toasty SQLite Prepared 主链及 callable 测试 SPI 已闭合；SQLx/RBDC 原生 generic execute、多数据库多结果、真实存储过程与 vendor generated-key 描述待补 |
| CORE-CONN-013 | `PreparedStatement#setXxx/clearParameters` 参数状态 | 48 个 setter 重载入口覆盖 null/typeName、标量、Decimal、日期时间/Calendar、Object 目标类型/scale、Stream/Reader int/long/无长度、LOB/Array/Ref/URL/RowId/SQLXML；1-based 稀疏参数在执行前拒绝；重复 setter 覆盖槽位；clear 仅在物理 clear 成功后清空；fatal setter 立即 discard；`JdbcParameter` 九对象合同及 0-based Proxy 参数视图已接；真实 Toasty/SQLx SQLite 与 RBDC 严格 SPI 已验证资源 setter 时点物化、query/update 和有序 batch | PARTIAL：JdbcParameter Java diff、RBDC 组合实库、SQLx Any 与 PostgreSQL/MySQL/Turso 资源及 vendor 类型矩阵待补 |
| CORE-CONN-014 | Statement/PreparedStatement query 保留驱动 ResultSet 身份 | `fetch_result_set/fetch_prepared_result_set/fetch_prepared_parameters_result_set` 贯穿物理 SPI、外部 lease、Filter、共享 Statement 状态和池化 wrapper；SQLx 真实 SQLite 普通/零行/prepared alias、RBDC driver label、默认 SPI 与 lease 均已测；Filter after 对未 eager 消费的结果集保持 `row_count=None` | PARTIAL：Toasty 明确为 eager RowSet；RBDC 零行 descriptor、streaming、PostgreSQL/MySQL/Turso 与 vendor ResultSet 尚未闭合 |

#### Filter/Proxy

| ID | Java 语义 | 状态 |
| :--- | :--- | :--- |
| CORE-FLT-001 | before 顺序/短路 | PARTIAL |
| CORE-FLT-002 | after 逆序/错误上下文 | PARTIAL |
| CORE-FLT-003 | connection 全 hook | TODO/PARTIAL |
| CORE-FLT-004 | statement/prepared/callable/resultset 全 hook | TODO |
| CORE-FLT-005 | Stat/Wall/Log/Config Filter | PARTIAL |
| CORE-FLT-006 | proxy attributes/raw/wrapper | TODO |
| CORE-FLT-007 | ResultSet 标量 getter 的精确重载、注册顺序、短路、返回改写与错误展开 | PASS（18 个 index/label 重载；真实 Toasty SQLite + 物理探针） |
| CORE-FLT-008 | ResultSet BigDecimal/Date/Time/Timestamp 重载及 Calendar 三态 Filter | PASS（16 个精确重载；真实 Toasty SQLite + 物理探针） |
| CORE-FLT-009 | ResultSet `getObject` 的 plain/map/typed × index/label 六重载 FilterChain | PASS（六个精确重载；nullable Map、目标类型、错误分类与真实 Toasty SQLite） |
| CORE-FLT-010 | ResultSet Ref/Blob/Clob/Array/URL/RowId/NClob/SQLXML 与五类 stream 的 index/label FilterChain | PASS（13 族 26 个精确重载；标签物理 SPI、共享资源句柄、短路、错误分类与真实 Toasty SQLite 流读取） |
| CORE-FLT-011 | ResultSet navigation/property 方法族 FilterChain | PASS（26 个方法；游标状态、int 参数、findColumn 标签、可失败 isClosed、错误分类与真实 Toasty SQLite 状态机） |
| CORE-FLT-012 | ResultSet `getNString` index/label 独立 FilterChain | PASS（两条精确重载；不再折叠为 getString，Unicode、短路与错误分类经真实 Toasty SQLite 验证） |
| CORE-FLT-013 | ResultSet `getMetaData()` 平台句柄 FilterChain | PASS（拥有物理 SPI 的 metadata 对象；默认穿透、短路、错误分类与真实 Toasty SQLite） |
| CORE-FLT-014 | ResultSet `getStatement()` 动态平台句柄 FilterChain | PASS（普通/Prepared/Callable 三身份；同一逻辑对象、短路替换、错误分类与真实 Toasty SQLite） |
| CORE-FLT-015 | ResultSet row-mutation 无参方法族 FilterChain | PASS（insert/update/delete/refresh/cancel/moveToInsert/moveToCurrent 七调用；默认穿透、短路、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-016 | ResultSet 基础列更新 setter FilterChain | PASS（updateNull 及 13 个 typed setter 的 index/label 共 28 个精确入口；默认穿透、全量短路、参数身份、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-017 | ResultSet `updateObject` 四重载 FilterChain | PASS（plain/scaleOrLength × index/label 四个精确入口；SQL NULL、vendor custom 对象身份、负 scale、短路、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-018 | ResultSet 资源对象 update setter FilterChain | PASS（Ref/Blob/Clob/Array/RowId/NClob/SQLXML × index/label 共 14 个精确入口；nullable、RowId 值、短路、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-019 | ResultSet LOB stream/Reader update setter FilterChain | PASS（Blob InputStream、Clob/NClob Reader × index/label × 无长度/long 共 12 个精确入口；nullable、共享游标、原始 long、短路、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-020 | ResultSet ASCII/Binary/Character/NCharacter stream update setter FilterChain | PASS（ASCII/Binary/Character × index/label × 无长度/int/long 18 个入口，加 NCharacter × index/label × 无长度/long 四个入口，共 22 个；nullable、共享游标、int/long 边界、短路、错误分类与真实 Toasty SQLite capability error） |
| CORE-FLT-021 | ResultSet `updateNString` index/label 独立 FilterChain | PASS（两条精确重载；nullable、Unicode、短路、错误分类与真实 Toasty SQLite generic update capability error） |
| CORE-FLT-022 | canonical `FilterAdapter` 默认适配对象 | PARTIAL（独立对象已建立；生命周期/配置空语义、精确自身 Wrapper、SQL before/after 与 185 个 ResultSet 默认继续链经真实 Toasty SQLite 验证；Java 491 个公开方法中的其余 hook 随 Filter/FilterChain 迁移继续开放） |
| CORE-FLT-023 | canonical `FilterEventAdapter` 事件模板对象 | PARTIAL（独立对象已建立；物理 connection_connect 已接直接/后台 creator，真实 ID、before→factory→after 反向顺序和错误主链已落；dataSource get/release 均为可短路的有位置 around-chain；当前生产链可承载的 Statement create/prepare/call、execute/query/update/batch、error-after 与 ResultSet open 已验证；StatementProxy/具体 DataSource 身份、downstream-before 局部展开及完整 Java 测试矩阵仍开放） |
| CORE-FLT-024 | canonical `FilterManager` alias/resource/factory 管理对象 | PARTIAL（独立对象与 Java bundled resource 已建立；properties 顺序覆盖、UTF-16 `<128` 回退、逗号展开、类名去重、缺失继续、构造失败及显式工厂加载已验证；自动 classloader 与 DataSource `setFilters` 装配仍开放） |

#### SQL/Wall/Stat

| ID | Java 语义 | 状态 |
| :--- | :--- | :--- |
| CORE-SQL-001 | 全 SQL AST 对象 | TODO/PARTIAL |
| CORE-SQL-002 | 方言 parser 与 feature | TODO/PARTIAL |
| CORE-SQL-003 | Visitor、clone、parent、attributes | TODO |
| CORE-SQL-004 | canonical output/format | TODO |
| CORE-SQL-005 | parameterize/merge/fingerprint | PARTIAL |
| CORE-WALL-001 | WallConfig 全字段 | PARTIAL（2026-08-13：use/show/describe/call/intersect/条件语义族/variant/object/read-only/检查门已接线并有 45 差分测试；wall_violation 100% 行覆盖） |
| CORE-WALL-002 | rule/violation/cache/provider | PARTIAL |
| CORE-STAT-001 | SQL/dataSource/web/spring 统计 | PARTIAL |
| CORE-STAT-002 | reset/MBean/JSON 结果 | TODO |

#### Toasty 标准实现与扩展边界

| ID | 语义 | 当前证据 | 状态 |
| :--- | :--- | :--- | :--- |
| CORE-DRV-001 | 内置 factory 每次创建 raw connection | 直接 `Driver::connect`，代码禁止 `toasty::Db` | PASS |
| CORE-DRV-002 | native pool 唯一所有者 | core SQLite 纵向测试回收同一连接 | PASS |
| CORE-DRV-003 | Toasty SQL driver capability | SQLite 实测；PG/MySQL/Turso feature 已定义 | PARTIAL |
| CORE-DRV-004 | 非 SQL driver 不冒充 JDBC | factory 拒绝 `Capability.sql=false` | PASS |
| CORE-DRV-005 | SQLx/RBDC 仍可通过唯一 `druid-wrapper` 扩展 | 已归入 wrapper 内部模块，回归通过 | PASS（架构边界） |
| CORE-DRV-006 | vendor error/generated keys/type metadata | SQLite 子集；多驱动未闭合 | PARTIAL |

#### SEM-NFR-*（全 workspace 非功能语义）

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

#### SEM-CFG-*

| ID | Java 行为 | Rust 目标 | 当前 | 验收 |
| :--- | :--- | :--- | :--- | :--- |
| SEM-CFG-001 | `DruidDataSource.init()` 幂等初始化 | `DruidDataSource::init()` + `DruidPool` lifecycle lock/initialized gate | IMPLEMENTED_UNVERIFIED | Java 并发 init、初始化失败重试和取消的统一差分 |
| SEM-CFG-002 | `configFromProperties` 属性名、别名、优先级 | serde/config facade | TODO | 全量属性 golden test |
| SEM-CFG-003 | 默认值与 Druid 1.2.28 一致 | config types | PARTIAL | 字段逐项 snapshot |
| SEM-CFG-004 | 非法 max/min/wait/validation 配置立即失败 | builder/init validation | PARTIAL | 错误类型和时机一致 |
| SEM-CFG-005 | SPI Filter/Checker/Sorter/Driver 自动发现和排序 | inventory registry | TODO | provider 顺序与 fallback |
| SEM-CFG-006 | init 创建 initialSize 连接并填充 holder | native pool 已按 `PhysicalConnectionInfo` 执行 driver connect→默认属性 init→有序 init SQL→MySQL session/global variables 采集→未执行 SQL时 factory validate→holder | PARTIAL/IMPLEMENTED_UNVERIFIED | 成功/部分失败/全失败、MySQL 变量、阶段错误计数和真实 SQLite/MySQL 统一验证 |
| SEM-CFG-007 | init 启动 creator/destroy/log tasks | 每池唯一受监管 create worker + close worker + maintenance task + stat publisher | PARTIAL/IMPLEMENTED_UNVERIFIED | 启停、creator 多等待者/失败恢复/合并请求、sink panic/error、取消、重启及周期字段差分 |
| SEM-CFG-008 | enable/disable/close/restart 状态门禁 | `PoolInner` enable/closed/generation/creatingCount + `DruidPool` lifecycle lock + canonical facade | PARTIAL/IMPLEMENTED_UNVERIFIED | Java 每状态 live diff；`restart(Properties)` 动态配置仍缺 |
| SEM-CFG-009 | 密码/配置版本变化使旧连接失效 | `notify_credentials_changed` + pool/holder 原子版本；不复制密码 | PARTIAL/IMPLEMENTED_UNVERIFIED | 归还时 discard、创建中切换、取消安全 |
| SEM-CFG-010 | MBean 配置读写结果 | admin config API | TODO | 字段、权限、错误对照 |
| SEM-CFG-011 | `initPhysicalConnection` 按 autoCommit → readOnly → isolation → catalog 初始化 | `PoolInner::initialize_physical_connection` | PARTIAL | 不同初始状态、空 catalog、unsupported |
| SEM-CFG-012 | 物理连接初始化失败时关闭连接、释放容量并记 connect error | native create failure path | PARTIAL | 每个 setter 失败、close 失败、并发关闭 |
| SEM-CFG-013 | `resetStatEnable` 默认 true；禁用时 `resetStat()` 完全无副作用 | `DruidPool` 原子 gate/count + `PoolInner#reset_stats` | IMPLEMENTED_UNVERIFIED | Java reset 前后字段 snapshot |

#### SEM-POOL-*

| ID | 语义 | 当前 | 关键差分场景 |
| :--- | :--- | :--- | :--- |
| SEM-POOL-001 | 优先复用 idle holder | PARTIAL | FIFO/LIFO 与时间字段 |
| SEM-POOL-002 | 空闲为空且容量可用时原子预留并创建 | PASS | 高并发不得越过 maxActive |
| SEM-POOL-003 | 达上限后按公平性等待 | PARTIAL（平台等价注记：parking_lot 非公平互斥 = Java useUnfairLock 默认非公平；公平模式为 JVM ReentrantLock 特有） | fair/unfair 唤醒顺序 |
| SEM-POOL-004 | maxWait 超时及错误字段 | IMPLEMENTED_UNVERIFIED | Java message/cause、毫秒边界 |
| SEM-POOL-005 | 创建失败退避、重试和 failFast | PARTIAL/IMPLEMENTED_UNVERIFIED | Java 多等待者、零/正退避、失败恢复 |
| SEM-POOL-006 | testOnBorrow | PARTIAL | valid/invalid/validator error |
| SEM-POOL-007 | testWhileIdle | IMPLEMENTED_UNVERIFIED | idle 时间阈值 |
| SEM-POOL-008 | keepAlive 探测与补最小空闲 | IMPLEMENTED_UNVERIFIED（2026-08-13 差分证据：maintenance_semantics_test 验证 minIdle 守恒与失败计数） | minIdle 守恒 |
| SEM-POOL-009 | 连接被 disable/closed datasource 拒绝 | IMPLEMENTED_UNVERIFIED | Java 错误类型 |
| SEM-POOL-010 | dataSource get/release 与物理 connect Filter | PARTIAL/IMPLEMENTED_UNVERIFIED | Java 具体 DruidDataSource 引用 |
| SEM-POOL-011 | active/pooling/connect/wait 计数原子变化 | IMPLEMENTED_UNVERIFIED | Java 中断、关闭唤醒 |
| SEM-POOL-012 | async 取消不遗失容量 permit | IMPLEMENTED_UNVERIFIED | 在每个 await 点取消 |
| SEM-POOL-013 | 所有获取入口只返回 `DruidPooledConnection` | PASS | native、dynamic、bridge 类型一致 |
| SEM-POOL-014 | 单次 acquire 只选择 native 或 external bridge 一个 Provider | PARTIAL | 配置互斥；禁止 pool-in-pool |
| SEM-POOL-015 | Provider 返回带唯一回收权的连接租约 | PARTIAL | 成功、超时、取消、Provider 关闭 |

#### SEM-REC-*

| ID | 语义 | 当前 | 验收 |
| :--- | :--- | :--- | :--- |
| SEM-REC-001 | close/Drop 只归还一次 | PASS | 重复 close、move、panic |
| SEM-REC-002 | `dyn Pool` 路径同样归还 | PASS | trait-object route 与 concrete pool 一致 |
| SEM-REC-003 | 非 autoCommit 且非 readOnly 时 rollback | PARTIAL | rollback 成功/失败 |
| SEM-REC-004 | holder reset 顺序 | PARTIAL/IMPLEMENTED_UNVERIFIED | 状态逐项恢复、任一步失败 |
| SEM-REC-005 | testOnReturn；invalid 时 destroy 且不计 recycleError | PARTIAL | invalid、validator error、timeout |
| SEM-REC-006 | fatal/discard/closed 连接不回池 | IMPLEMENTED_UNVERIFIED | Java `OnFatalErrorMaxActiveTest*` |
| SEM-REC-007 | phyMaxUseCount、phyTimeout | PARTIAL | 边界时刻 |
| SEM-REC-008 | password/config version 淘汰 | PARTIAL/IMPLEMENTED_UNVERIFIED | 版本切换、创建中竞态 |
| SEM-REC-009 | 满池返还物理关闭且计数正确 | PARTIAL | 与并发 create 竞争 |
| SEM-REC-010 | removeAbandoned 追踪与关闭 | IMPLEMENTED_UNVERIFIED | owner、timeout、stack trace |
| SEM-REC-011 | shrink/idle eviction | IMPLEMENTED_UNVERIFIED | minIdle/keepAlive/fatalError 统一组合差分 |
| SEM-REC-012 | `tryGetConnection/isFull/fill()` 协调、restart | PARTIAL/IMPLEMENTED_UNVERIFIED | 维护操作、统计、动态配置 |
| SEM-REC-013 | creator/destroy task 关闭可等待 | IMPLEMENTED_UNVERIFIED | graceful shutdown |
| SEM-REC-014 | statement cache 清理 | PARTIAL | recycle error/discard/schema change |
| SEM-REC-015 | 一个 lease 只有一个归还策略且至多执行一次 | PASS | close、Drop、panic |
| SEM-REC-016 | async recycle 失败进入受监管回收 | IMPLEMENTED_UNVERIFIED | runtime 内外 Drop |
| SEM-REC-017 | native 归还 Druid idle queue；bridge 归还原外部池 | PASS | 两种模式计数和所有权分别守恒 |

#### SEM-CONN-*

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-CONN-001 | autoCommit/readOnly/isolation/catalog/schema/holdability | PARTIAL | getter/setter/unsupported |
| SEM-CONN-002 | commit、rollback、savepoint | PARTIAL | 成功、错误、状态 |
| SEM-CONN-003 | network/query timeout 与 cancel | TODO | driver cancellation |
| SEM-CONN-004 | statement execute/query/update/largeUpdate/batch | PARTIAL | 返回类型、行数、最近状态 Java live diff |
| SEM-CONN-005 | prepared 参数类型、null、LOB、清理和 batch | PARTIAL | JdbcParameter Java live/golden diff |
| SEM-CONN-006 | callable IN/OUT 参数 | PARTIAL | Java 101 个 callable oracle |
| SEM-CONN-007 | result set next/close/fetch row count/hold time | PARTIAL | streaming 提前退出 |
| SEM-CONN-008 | metadata、unwrap 与 Proxy identity/attributes | PARTIAL | Adapter vendor properties |
| SEM-CONN-009 | fatal exception 使 holder discard | PARTIAL | Java vendor 23/23 |
| SEM-CONN-010 | XA prepare/commit/rollback/recover | TODO | 2PC 故障注入 |
| SEM-CONN-011 | `PhysicalConnection` 只表达物理数据库能力 | PASS | API surface 与依赖检查 |
| SEM-CONN-012 | SQLx/RBDC 通过同一 direct contract | PARTIAL | reset、cancel、metadata |
| SEM-CONN-013 | bb8/deadpool 通过 Provider contract | PASS | acquire/timeout/return/broken lease |
| SEM-CONN-014 | adapter 统一上报 capability 与结构化 `PhysicalError` | TODO | unsupported、transient、fatal |
| SEM-CONN-015 | `PreparedStatement` 继承真实 `Statement` 属性 | PASS | 属性/恢复生命周期 |
| SEM-CONN-016 | Prepared 资源 setter 必须先调用物理句柄 | PASS（Toasty/SQLx SQLite、RBDC SPI） | xerial SQLite JShell |
| SEM-CONN-017 | DataSourceProxy 统计/驱动/URL/Filter/properties 与 ID 序列 | PARTIAL/IMPLEMENTED_UNVERIFIED | DataSourceProxyConfig callbacks/JMX |
| SEM-CONN-018 | ResultSetMetaData Proxy 身份 | PARTIAL/IMPLEMENTED_UNVERIFIED | metadata 21 方法专用 Filter |
| SEM-CONN-019 | DatabaseMetaData 176 声明 | PARTIAL/IMPLEMENTED_UNVERIFIED | 13 个平台/driver unsupported 边界 |

#### SEM-FLT-*

| ID | Java 语义 | Rust 当前 | 状态 | 验收 |
| :--- | :--- | :--- | :--- | :--- |
| SEM-FLT-001 | Filter 按配置顺序执行 before | `FilterChain::before_execute` | PARTIAL | 顺序/短路 |
| SEM-FLT-002 | after 在成功和失败均执行，顺序正确 | `after_execute(...).await` | PASS | exactly-once |
| SEM-FLT-003 | `exec` 全上下文透传 | before/after 复用同一 `ExecContext` | PASS | 参数、name、start、fingerprint |
| SEM-FLT-004 | `fetch` 同样经过 chain | 统一 before/after | PASS | before/after + row stat |
| SEM-FLT-005 | connection_* hooks | 事件 enum，仅部分调用 | PARTIAL | 每个 Java hook |
| SEM-FLT-006 | statement/prepared/callable hooks | 事件 enum | PARTIAL | 全执行入口 |
| SEM-FLT-007 | resultSet/LOB/metadata hooks | ResultSet next/close 已接同步 around-chain | PARTIAL | 全访问族与 close |
| SEM-FLT-008 | init/destroy/configFromProperties | canonical `FilterChainImpl` | PARTIAL/IMPLEMENTED_UNVERIFIED | 配置失败/生命周期时序统一差分 |
| SEM-FLT-009 | dynamic-dispatch chain 游标 | canonical `FilterChainImpl` 已落位 | PARTIAL | 补齐其余对象族 |
| SEM-FLT-010 | Filter alias/SPI/order | canonical `FilterManager` + bundled aliases | PARTIAL/IMPLEMENTED_UNVERIFIED | JVM 三 ClassLoader 自动资源扫描 |
| SEM-FLT-011 | WallFilter 接入 | canonical `WallFilter` | PARTIAL/IMPLEMENTED_UNVERIFIED | 四种 log/throw 组合 |
| SEM-FLT-012 | StatFilter 接入 | SQL before/after、batch、事务、ResultSet、pool、physical connect/close 已接 | PARTIAL | LOB、慢参数文本 |
| SEM-FLT-013 | LogFilter 输出配置 | canonical tracing-backed `LogFilter` | PARTIAL/IMPLEMENTED_UNVERIFIED | 完整 ResultSet 值 |
| SEM-FLT-014 | ConfigFilter 解密/远程配置 | canonical `ConfigFilter` | PARTIAL/IMPLEMENTED_UNVERIFIED | Java/Rust 旧密文 live differential |
| SEM-FLT-015 | Encoding/DateTime filter 结果 | 独立 canonical 对象 | PARTIAL/IMPLEMENTED_UNVERIFIED | Prepared/Callable 参数描述符 |
| SEM-FLT-016 | execute/query future 被取消后仍产生唯一 completion outcome | 无 completion guard | TODO | 每个 await 点 |
| SEM-FLT-017 | `StatFilterContext` 进程单例 | `OnceLock` + COW | PARTIAL | Java JShell 动态增补 |
| SEM-FLT-018 | execute before/after 时序 | `ExecContext.in_transaction/operation` | PASS | Java StatFilter/MockDriver oracle |
| SEM-FLT-019 | 普通 Statement batch Filter 链 | `BatchExecContext` | PASS | Java MockDriver JShell |
| SEM-FLT-020 | PreparedStatement batch Filter 链 | `BatchExecKind::PreparedStatement` | PASS | Java Druid+xerial SQLite JShell |
| SEM-FLT-021 | 普通 Statement generic execute | `ExecOperation::Execute` | PASS | Java Druid+xerial SQLite JShell |
| SEM-FLT-022 | PreparedStatement generic execute | `PhysicalConnection::execute_prepared` | PASS | Java Druid+xerial SQLite JShell |
| SEM-FLT-023 | Connection/Statement/PreparedStatement/ResultSet warnings | 四类对象 warning FilterChain | PASS | CodeGraph Java 六条 warning 路径 |
| SEM-FLT-024 | ResultSet 标量 getter FilterChain | 18 条同名入口 | PASS | Java 18 条签名逐项对照 |
| SEM-FLT-025 | ResultSet BigDecimal/Date/Time/Timestamp FilterChain | 16 条强类型入口 | PASS | Java 16 条签名逐项对照 |
| SEM-FLT-026 | ResultSet getObject 六重载 FilterChain | 六条 object 入口 | PASS | Java 六条签名逐项对照 |
| SEM-FLT-027 | ResultSet stream/resource FilterChain | 26 条入口 | PASS | Java 26 条签名逐项对照 |
| SEM-FLT-028 | ResultSet navigation/property FilterChain | 26 个方法 | PASS | Java 26 条方法逐项对照 |
| SEM-FLT-029 | ResultSet getNString FilterChain | 两条精确入口 | PASS | Java 两条签名逐项对照 |
| SEM-FLT-030 | ResultSet getMetaData FilterChain | 四层精确入口 | PASS | Java 三层签名对照 |
| SEM-FLT-031 | ResultSet getStatement FilterChain | 四层精确入口 | PASS | Java Filter/FilterChain 静态签名 |
| SEM-FLT-032 | ResultSet row-mutation FilterChain | 七条精确入口 | PASS | Java 七签名静态对照 |
| SEM-FLT-033 | ResultSet 基础列更新 setter FilterChain | 28 条精确入口 | PASS | Java 四层 28 签名静态对照 |
| SEM-FLT-034 | ResultSet updateObject FilterChain | 四条精确入口 | PASS | Java 四重载静态对照 |
| SEM-FLT-035 | ResultSet 资源对象 update setter FilterChain | 14 条精确入口 | PASS | Java 四层 14 签名静态对照 |
| SEM-FLT-036 | ResultSet LOB stream/Reader update setter FilterChain | 12 条精确入口 | PASS | Java 12 签名静态对照 |
| SEM-FLT-037 | ResultSet stream update setter FilterChain | 22 条精确入口 | PASS | Java 22 签名静态对照 |
| SEM-FLT-038 | ResultSet updateNString FilterChain | 两条精确入口 | PASS | Java 两签名静态对照 |
| SEM-FLT-039 | canonical `FilterAdapter` 默认适配语义 | 独立 `filter_adapter.rs` | PARTIAL | Java 491 个公开方法 |
| SEM-FLT-040 | canonical `FilterEventAdapter` 事件模板 | 独立 `filter_event_adapter.rs` | PARTIAL | Java 541 行、32 个公开入口 |
| SEM-FLT-041 | canonical `FilterManager` alias/resource/factory | `filter_manager.rs` + bundled properties | PARTIAL | Java `FilterManagerTest` |
| SEM-FLT-042 | `Filter#connection_close` 包围 raw driver close | `PhysicalConnectionCloseFilterChain` | PARTIAL/IMPLEMENTED_UNVERIFIED | Java Filter/FilterChainImpl 源码顺序 |
| SEM-FLT-043 | `Filter#connection_connect` 包围 raw driver connect | `PhysicalConnectionConnectFilterChain` | PARTIAL/IMPLEMENTED_UNVERIFIED | Java FilterChainImpl/StatFilter 源码顺序 |

#### SEM-SQL-*

| ID | 语义 | Rust 迁移策略 | 当前 | 验收 |
| :--- | :--- | :--- | :--- | :--- |
| SEM-SQL-001 | `DbType.of` 名称/别名、mask/hash/equals/style | canonical `DbType` + 分层 `JdbcUtils` | PARTIAL/IMPLEMENTED_UNVERIFIED | Java 全枚举 live/golden fixture |
| SEM-SQL-001A | `JdbcUtils.getDbTypeRaw/getDbType/getTypeName` | 严格 JDBC 兼容入口 + 独立 Rust `infer_db_type` | PARTIAL/IMPLEMENTED_UNVERIFIED | Java URL/type/family 全矩阵 |
| SEM-SQL-002 | Lexer token、offset、line/column | `LayoutCharacters`、`CharTypes`、335 项 `Token`、`Keywords` 与 canonical `Lexer` | PARTIAL/IMPLEMENTED_UNVERIFIED | Token/keyword/hash Java live diff |
| SEM-SQL-002A | parser 异常类型、消息、位置与 cause | canonical `ParserException` + EOF/NotAllowComment/SQLParseException | IMPLEMENTED_UNVERIFIED | 构造器/class/message/source Java live diff |
| SEM-SQL-002B | `SymbolTable` intern 与 hash 冲突 | canonical `SymbolTable` | IMPLEMENTED_UNVERIFIED | identity、collision Java live diff |
| SEM-SQL-002C | `SQLInsertValueHandler` 流式 VALUES 回调 | 关联 Row 保留自定义行身份 | IMPLEMENTED_UNVERIFIED | 回调次序、index、类型分支 Java live diff |
| SEM-SQL-003 | 注释、hint、变量和字符串转义 | canonical Lexer 已迁移基础转义 | PARTIAL/IMPLEMENTED_UNVERIFIED | 保留/跳过/拒绝 Java live diff |
| SEM-SQL-004 | `SQLParserFeature` 开关 | canonical `SqlParserFeature` 独立对象 | IMPLEMENTED_UNVERIFIED | 29 项 ordinal/mask/name |
| SEM-SQL-004B | `DialectFeature` lexer/parser 双 mask | canonical `DialectFeature` | IMPLEMENTED_UNVERIFIED | 全 mask/name、alias |
| SEM-SQL-004A | `SQLType` 完整分类返回域 | canonical `SqlType` 独立对象 | IMPLEMENTED_UNVERIFIED | 128 项 ordinal/name |
| SEM-SQL-005 | `parseStatements` 全输入消费与 EOF | `SqlUtils::parse_statements/parse_single_statement` | PARTIAL/IMPLEMENTED_UNVERIFIED | trailing token、空输入 |
| SEM-SQL-006 | 多语句、分隔符与 keepComments | `SqlUtils::parse_statements/to_sql_string` | PARTIAL/IMPLEMENTED_UNVERIFIED | statement list/output diff |
| SEM-SQL-007 | 方言 parser 工厂/SPI | registry + sqlparser 扩展 | TODO | DbType→parser |
| SEM-SQL-008 | AST 节点类型、字段与 parent | compatibility AST | TODO | 逐对象 snapshot |
| SEM-SQL-009 | attributes、before/after comments | metadata sidecar | TODO | clone/serialize |
| SEM-SQL-010 | Visitor visit/endVisit 和中止 | visitor trait | TODO | 事件序列 |
| SEM-SQL-011 | output visitor 格式化 | formatter adapter | TODO | byte/golden diff |
| SEM-SQL-012 | parameterize、参数列表和 merge | `SqlUtils::parameterize` + `SqlMerger` | PARTIAL/IMPLEMENTED_UNVERIFIED | SQL+params diff |
| SEM-SQL-013 | fingerprint 稳定性 | canonical hash policy | PARTIAL | Java key 对照 |
| SEM-SQL-014 | schema repository/resolve | repository module | TODO | DDL→lineage |
| SEM-SQL-015 | builder/transform | AST rewrite | TODO | rewrite diff |
| SEM-SQL-016 | `PagerUtils` count/limit | dialect pager | TODO | 方言 SQL diff |
| SEM-SQL-017 | parser exception 类型、位置、消息 | `DruidSqlError` | TODO | malformed corpus |
| SEM-SQL-018 | MySQL 方言对象族 | extension/fork | TODO | Java MySQL corpus |
| SEM-SQL-019 | Oracle 方言对象族 | extension/fork | TODO | Java Oracle corpus |
| SEM-SQL-020 | PostgreSQL 方言对象族 | extension/fork | TODO | Java PG corpus |
| SEM-SQL-021 | SQL Server/DB2/ClickHouse 等方言 | extension/fork | TODO | 各自 corpus |

#### SEM-WALL-*

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-WALL-001 | provider 根据 DbType 选择，未知方言走 SPI | TODO | provider 矩阵 |
| SEM-WALL-002 | select/insert/update/delete allow | PARTIAL | 每项开/关 |
| SEM-WALL-003 | DDL、transaction、set/show/use 等 allow | PARTIAL（show/use/describe/call/intersect/START/EXPLAIN 已接线并测试） | 每项开/关 |
| SEM-WALL-004 | update/delete 必须有 where | PARTIAL | AST 语义 |
| SEM-WALL-005 | 恒真/恒假/double const/like/case 条件 | PARTIAL（AlwaysFalse/DoubleConst/Xor/Bitwise/ConstArithmetic/SameConstLike/ConstCase 七类已按 Java getConditionValue/getValue_and 接线并差分测试） | Java Wall corpus |
| SEM-WALL-006 | deny/permit table/function/schema/variant | PARTIAL/IMPLEMENTED_UNVERIFIED | 大小写、schema、嵌套、permit 优先级 |
| SEM-WALL-007 | white list 与 cache | PARTIAL | hit/miss/reset |
| SEM-WALL-008 | multiStatement/comment/hint/metadata | PARTIAL/IMPLEMENTED_UNVERIFIED | parser feature 组合 |
| SEM-WALL-009 | mustParameterized | TODO | 参数化规则 |
| SEM-WALL-010 | tenant column/table pattern 注入、隐藏与检查 | PARTIAL/IMPLEMENTED_UNVERIFIED | Java TenantSelect/Update/Insert 全 corpus |
| SEM-WALL-011 | WallContext 和 check 统计 | PARTIAL/IMPLEMENTED_UNVERIFIED | Java ThreadLocal 生命周期 |
| SEM-WALL-012 | violation 类型、errorCode、message | PARTIAL | 逐规则对照 |
| SEM-WALL-013 | logViolation/throwException | PARTIAL | 四种组合 |
| SEM-WALL-014 | provider cache clear/white list API | PARTIAL/IMPLEMENTED_UNVERIFIED | `WallStatTest_statMap` |
| SEM-WALL-015 | 各方言 WallVisitor | PARTIAL/IMPLEMENTED_UNVERIFIED | 七方言 Java 安全语料 |
| SEM-WALL-016 | updateCheckColumns 与 Prepared 参数检查 | PARTIAL/IMPLEMENTED_UNVERIFIED | `WallUpdateCheckTest` 全 case |

#### SEM-STAT-*

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-STAT-001 | SQL 参数化后合并 | PARTIAL | 基于 P4 AST 与 Java key 对照 |
| SEM-STAT-002 | execute/error/running/concurrentMax | PARTIAL/IMPLEMENTED_UNVERIFIED | 固定时钟并发、错误字段 golden |
| SEM-STAT-003 | execute time total/max/histogram | PARTIAL/IMPLEMENTED_UNVERIFIED | 固定时钟边界桶 |
| SEM-STAT-004 | update/fetch rows total/max/直方图 | PARTIAL/IMPLEMENTED_UNVERIFIED | Java `JdbcSqlStat#addUpdateCount/addFetchRowCount` |
| SEM-STAT-005 | transaction/batch/resultSet hold time | PARTIAL/IMPLEMENTED_UNVERIFIED | 状态序列 |
| SEM-STAT-006 | slow SQL/slow parameters | PARTIAL/IMPLEMENTED_UNVERIFIED | `StatFilterBuildSlowParameterTest` cases |
| SEM-STAT-007 | datasource/connection/statement/resultset 分层统计 | PARTIAL/IMPLEMENTED_UNVERIFIED | 快照 41 键 Java golden/live diff |
| SEM-STAT-008 | reset 的原子语义 | PARTIAL/IMPLEMENTED_UNVERIFIED | 与并发 record 竞争 |
| SEM-STAT-009 | StatFilter 上下文/listener | PARTIAL/IMPLEMENTED_UNVERIFIED | 嵌套调用、短路与首错 |
| SEM-STAT-010 | JMX 属性映射 OpenMetrics | TODO | 指标名、label、类型 |
| SEM-STAT-011 | JdbcStatContext 执行上下文 | PARTIAL/IMPLEMENTED_UNVERIFIED | task 终结清理 |
| SEM-STAT-012 | TableStat 与 inner 值对象 | IMPLEMENTED_UNVERIFIED | Java FNV/normalize |
| SEM-STAT-011 | DruidStatService JSON schema | PARTIAL/IMPLEMENTED_UNVERIFIED | golden response |
| SEM-STAT-012 | tracing/logging 字段和脱敏 | PARTIAL | event snapshot |

#### SEM-HA-*

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-HA-001 | named/random/sticky selector | PARTIAL/IMPLEMENTED_UNVERIFIED | Java selector 全矩阵 |
| SEM-HA-002 | 节点校验、摘除、恢复 | PARTIAL/IMPLEMENTED_UNVERIFIED | Java raw-driver 与 pooled recover 差异 |
| SEM-HA-003 | Registry/Watcher 事件 | PARTIAL/IMPLEMENTED_UNVERIFIED | 文件变化 Java live diff |
| SEM-HA-004 | 热切换与旧池排空 | PARTIAL/IMPLEMENTED_UNVERIFIED | 活跃事务/连接归还时间线 |
| SEM-HA-005 | 全节点不可用错误 | PARTIAL/IMPLEMENTED_UNVERIFIED | 空 map、HA blacklist |

---

## druid-admin 模块

### 对象级对照表

#### 生产对象 13/13

| Java 对象 | 目标 Rust 文件/对象 | 映射 | 当前 | 职责 |
| :--- | :--- | :--- | :--- | :--- |
| `DruidAdminApplication` | `druid_admin_application.rs` / `DruidAdminApplication` | ADAPTER | IMPLEMENTED_UNVERIFIED | Topcoat 启动、配置、Axum/Tower Router 装配 |
| `MonitorProperties` | `config/monitor_properties.rs` / `MonitorProperties` | DIRECT | IMPLEMENTED_UNVERIFIED | 服务名、登录、context path、namespace、kube config |
| `ServiceNode` | `model/service_node.rs` / `ServiceNode` | DIRECT | IMPLEMENTED_UNVERIFIED | serviceId、host、port、节点元数据；prost 快照 |
| `ConnectionResult` | `model/dto/connection_result.rs` / `ConnectionResult` | DIRECT | IMPLEMENTED_UNVERIFIED | 活跃/池化连接结果 |
| `DataSourceResult` | `model/dto/data_source_result.rs` / `DataSourceResult` | DIRECT | IMPLEMENTED_UNVERIFIED | 数据源统计字段 |
| `SqlDetailResult` | `model/dto/sql_detail_result.rs` / `SqlDetailResult` | DIRECT | IMPLEMENTED_UNVERIFIED | 单 SQL 详情 |
| `SqlListResult` | `model/dto/sql_list_result.rs` / `SqlListResult` | DIRECT | IMPLEMENTED_UNVERIFIED | SQL 列表/聚合结果 |
| `WallResult` | `model/dto/wall_result.rs` / `WallResult` | DIRECT | IMPLEMENTED_UNVERIFIED | Wall 统计与 sum |
| `WebResult` | `model/dto/web_result.rs` / `WebResult` | DIRECT | IMPLEMENTED_UNVERIFIED | WebApp/URI/session 结果 |
| `K8sDiscoveryClient` | `service/k8s_discovery_client.rs` / `K8sDiscoveryClient` | ADAPTER | IMPLEMENTED_UNVERIFIED | Kubernetes pod 发现，生产实现为 kube-rs provider |
| `MonitorStatService` | `service/monitor_stat_service.rs` / `MonitorStatService` | DIRECT/ADAPTER | IMPLEMENTED_UNVERIFIED | 远端请求、聚合、排序、分页、JSON |
| `MonitorViewServlet` | `servlet/monitor_view_servlet.rs` / `MonitorViewServlet` | PROTOCOL | IMPLEMENTED_UNVERIFIED | Servlet → Axum 路由、静态兼容面、登录/session |
| `HttpUtil` | `util/http_util.rs` / `HttpUtil` | DIRECT/ADAPTER | IMPLEMENTED_UNVERIFIED | reqwest GET、JSON 反序列化、typed error |

#### Rust 当前对象

| Rust 对象 | Java 对应 | 判断 |
| :--- | :--- | :--- |
| `AdminState` | 无直接单一对象；Router state | RUST_ONLY 兼容门面 |
| `DiscoveryClient` / `ServiceInstance` | Spring Cloud discovery protocol | ADAPTER SPI |
| `KubeRsDiscoveryProvider` | Kubernetes Java client | ADAPTER，使用 kube-rs |
| `ReqwestHttpClient` | Java `HttpUtil` 底层客户端 | ADAPTER |
| `StatQuery` / `LoginRequest` | Servlet 参数 | ADAPTER |
| `endpoint_list()` | Rust 早期门面 | RUST_ONLY 清单 |

#### 资源账本

| 资源族 | Java 位置 | Rust 目标 | 当前 |
| :--- | :--- | :--- | :--- |
| HTML 页面 | `support/http/resources/*.html` | 同路径迁入并编译期嵌入 | 18/18 BYTE_IDENTICAL / 路由未验证 |
| CSS/JS | `support/http/resources/css|js` | 同路径迁入并由 Axum 静态兼容路由暴露 | 7/7 BYTE_IDENTICAL / 路由未验证 |
| monitor index | `support/monitor/resources/index.html` | Topcoat 根页面 | IMPLEMENTED_UNVERIFIED |
| MySQL monitor SQL | `support/monitor/mysql/*.sql` | `resources/support/monitor/mysql/` | 8/8 IMPLEMENTED_UNVERIFIED |
| `bootstrap.yml` | resources root | `MonitorProperties` typed config | ADAPTER/IMPLEMENTED_UNVERIFIED |

#### 平台依赖映射

| Java/Spring 能力 | Rust 能力 | 规则 |
| :--- | :--- | :--- |
| Spring Boot application | Topcoat server + library Router | 不把启动器混入领域对象 |
| `ResourceServlet` | Topcoat 外壳 + Axum handler | 路由结果一致 |
| Servlet 参数校验 | axum-valid | Form/Query 在边界校验 |
| Servlet runtime | Tokio | 异步任务、取消与关停进入统一验证 |
| Servlet task statistics | tokio-metrics | `/metrics` |
| 节点快照传输 | prost | Rust-only 可选能力 |
| 管理 UI | Topcoat + Axum HTML | 功能语义迁移 |
| Spring Cloud Discovery | `DiscoveryClient` trait + adapters | 每种注册中心独立实现 |
| Kubernetes Java client | `kube`/HTTP adapter | namespace/label/error 一致 |
| Apache HttpClient/OkHttp | typed async HTTP client | timeout/状态码/JSON 错误一致 |
| Fastjson2 | serde/serde_json | 字段名、null、数字和数组一致 |

### 语义迁移对照表

#### 配置与发现

| ID | Java 语义 | Rust 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| ADM-CFG-001 | MonitorProperties 默认值/绑定 | serde typed config | IMPLEMENTED_UNVERIFIED |
| ADM-DISC-001 | 按 serviceNames 发现节点 | `DiscoveryClient` SPI | IMPLEMENTED_UNVERIFIED |
| ADM-DISC-002 | namespace/kubeconfig | kube-rs 自定义 kubeconfig | IMPLEMENTED_UNVERIFIED |
| ADM-DISC-003 | 多注册中心来源 | 外部 `DiscoveryClient` Adapter SPI | IMPLEMENTED_UNVERIFIED |
| ADM-DISC-004 | 节点去重与 serviceIdMap | `serviceName-address-port` 去重 | IMPLEMENTED_UNVERIFIED |

#### 监控服务

| ID | Java 语义 | Rust 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| ADM-SVC-001 | URL 分派 `service(String)` | 完整 legacy dispatcher + Axum fallback | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-002 | `returnJSONResult(code, content)` | `return_json_result` | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-003 | Wall 统计合并 | 数值求和、数组追加 | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-004 | SQL list/detail | 多节点列表、详情转发 | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-005 | datasource 统计 | 全节点聚合 | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-006 | pooling connection info | serviceId 索引 | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-007 | WebURI/session/app/spring 统计 | 远端 WebURI + 本地 `AdminStatProvider` | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-008 | `orderBy`/`orderType`/`page`/`perPageCount` | Java 默认值、排序分页 | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-009 | query 参数解析与 URL 构造 | 保留 Java raw parser | IMPLEMENTED_UNVERIFIED |
| ADM-SVC-010 | 部分节点错误和空结果 | 聚合并跳过失败节点 | IMPLEMENTED_UNVERIFIED |

#### HTTP/资源

| ID | Java 语义 | Rust 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| ADM-HTTP-001 | `/druid/*` 路由 | Topcoat `TowerRoute` 挂载 Axum Router | IMPLEMENTED_UNVERIFIED |
| ADM-HTTP-002 | 静态资源、content-type、缓存 | 25/25 原资源逐字节迁入 | IMPLEMENTED_UNVERIFIED |
| ADM-HTTP-003 | JSON status/body | Java compatibility JSON | IMPLEMENTED_UNVERIFIED |
| ADM-HTTP-004 | login/nopermit 页面 | 302 登录跳转、Form 校验、session cookie | IMPLEMENTED_UNVERIFIED |
| ADM-HTTP-005 | HTTP client timeout/status/deserialize | reqwest connect/request timeout | IMPLEMENTED_UNVERIFIED |

#### DTO

| ID | Java 语义 | 状态 |
| :--- | :--- | :--- |
| ADM-DTO-001 | 6 个顶层 DTO、内部 Bean 字段名与 nullability | IMPLEMENTED_UNVERIFIED |
| ADM-DTO-002 | 数值范围和 JSON number | IMPLEMENTED_UNVERIFIED |
| ADM-DTO-003 | `WallResult#sum` 聚合口径 | IMPLEMENTED_UNVERIFIED |
| ADM-DTO-004 | 序列化字段顺序/快照 | TODO |

#### SEM-ADM-* / SEM-INT-*

| ID | 语义 | 原总账状态 | 验收条件 |
| :--- | :--- | :--- | :--- |
| SEM-ADM-001 | datasource/sql/wall/web/session 端点 | IMPLEMENTED_UNVERIFIED | JSON schema/status |
| SEM-ADM-002 | 登录、IP allow/deny、session | PARTIAL | Java admin 当前只装配 login/session |
| SEM-ADM-003 | reset/clear/enable/disable 管理动作 | TODO | 权限和副作用 |
| SEM-ADM-004 | 静态资源与页面 | IMPLEMENTED_UNVERIFIED | 25/25 byte-identical |
| SEM-INT-001 | starter 配置绑定和条件装配结果 | TODO | config fixtures |
| SEM-INT-002 | WebStatFilter 请求/URI/session 统计 | TODO | Tower middleware |
| SEM-INT-003 | ORM/iBatis SQL 统计集成 | TODO | ORM fixture |

#### 管理端点

| Java 端点/对象 | Rust 目标 | 当前 | 完成条件 |
| :--- | :--- | :--- | :--- |
| `StatViewServlet` | Axum `StatViewRouter` | TODO | route、认证、IP |
| `/druid/index.html` | `/druid/index.html` | IMPLEMENTED_UNVERIFIED | 功能页面 |
| `/druid/datasource.json` | `/druid/api/datasources` + compatibility route | IMPLEMENTED_UNVERIFIED | JSON 字段 |
| `/druid/sql.json` | `/druid/api/sql/top` + compatibility route | IMPLEMENTED_UNVERIFIED | 排序、筛选、分页 |
| `/druid/wall.json` | `/druid/api/wall` | IMPLEMENTED_UNVERIFIED | Wall stat/config |
| `/druid/connectionInfo-*.json` | legacy fallback + `/druid/api/active` | IMPLEMENTED_UNVERIFIED | serviceId/id |
| `/druid/weburi.json` | URI stat API | IMPLEMENTED_UNVERIFIED | `AdminStatProvider`/远端聚合 |
| `/druid/webapp.json` | app stat API | IMPLEMENTED_UNVERIFIED | application snapshot |
| `/druid/websession.json` | session stat API | IMPLEMENTED_UNVERIFIED | session adapter |
| `/druid/spring.json` | integration stat API | IMPLEMENTED_UNVERIFIED | Rust integration 对应结果 |
| `WebStatFilter` | `WebStatLayer` | TODO | request/URI/session |
| `DruidStatServiceMBean` | metrics + admin operations | TODO | 属性和操作 |

#### Spring Boot Starter 结果迁移

| Java Starter 对象 | Rust 目标 | 当前 |
| :--- | :--- | :--- |
| `DruidDataSourceAutoConfigure` | feature + config loader + builder | TODO |
| `DruidDataSourceWrapper` | `DruidDataSource` facade | TODO |
| `DruidStatProperties` | typed config | TODO |
| `DruidFilterConfiguration` | filter registry/config | TODO |
| `DruidStatViewServletConfiguration` | admin router config | TODO |
| `DruidWebStatFilterConfiguration` | Tower layer config | TODO |
| `DruidSpringAopConfiguration` | tracing/instrumentation layer | TODO |
| `spring.datasource.druid.*` | compatible alias profile | TODO |

---

## druid-wrapper 模块

### 对象级对照表

#### Java 13 个对象

| Java 对象 | Java 职责 | Rust 迁移落点 | 决策 | 当前 |
| :--- | :--- | :--- | :--- | :--- |
| `com.mchange.v2.c3p0.PooledDataSource` | c3p0 DataSource 类型兼容 | `Pool` provider contract | ADAPTER | PARTIAL |
| `com.mchange.v2.c3p0.ComboPooledDataSource` | Druid c3p0 facade | `SqlxBb8Pool`/`SqlxDeadpoolPool` + config facade | ADAPTER | PARTIAL |
| `org.apache.commons.dbcp.BasicDataSource` | DBCP facade | `DruidPool`/external provider facade | ADAPTER | PARTIAL |
| `org.apache.commons.dbcp.BasicDataSourceFactory` | properties 创建 datasource | `WrapperDataSourceFactory` | DIRECT/ADAPTER | MISSING |
| `org.apache.commons.dbcp.BasicDataSourceMBean` | 管理属性 | `WrapperPoolState`/metrics | PROTOCOL | MISSING |
| `org.apache.commons.dbcp.ManagedBasicDataSource` | managed facade | `ManagedWrapperPool` | ADAPTER | MISSING |
| `org.apache.commons.dbcp.ManagedBasicDataSourceFactory` | managed factory | `ManagedWrapperPoolFactory` | ADAPTER | MISSING |
| `org.apache.commons.dbcp.ManagedBasicDataSourceMBean` | managed MBean | metrics/admin protocol | PROTOCOL | MISSING |
| `org.apache.commons.dbcp2.BasicDataSource` | DBCP2 facade | provider/config facade | ADAPTER | PARTIAL |
| `org.apache.commons.dbcp2.BasicDataSourceFactory` | DBCP2 properties factory | `WrapperDataSourceFactory` | MERGE/ADAPTER | MISSING |
| `org.apache.commons.dbcp2.BasicDataSourceMBean` | DBCP2 management | metrics/admin protocol | PROTOCOL | MISSING |
| `org.logicalcobwebs.proxool.ProxoolConstants` | Proxool property names/defaults | `ProxoolConfigKey`/`WrapperPoolConfig` | DIRECT/MERGE | MISSING |
| `org.logicalcobwebs.proxool.ProxoolDataSource` | Proxool properties/DataSource/JNDI | `ProxoolDataSourceAdapter` | ADAPTER | MISSING |

#### Rust Adapter 对象

| Rust 对象 | 当前文件 | 归并来源（历史） | 当前 |
| :--- | :--- | :--- | :--- |
| `SqlxConnectionAdapter` | `druid-wrapper/src/sqlx/sqlx_connection_adapter.rs` | `druid-sqlx` | PARTIAL |
| `SqlxDatabaseMetaData` | `druid-wrapper/src/sqlx/sqlx_database_meta_data.rs` | SQLx SQLite metadata Adapter | RUST_EXTENSION/IMPLEMENTED_UNVERIFIED |
| `SqlxConnectionFactory` | `druid-wrapper/src/sqlx/sqlx_connection_factory.rs` | `druid-sqlx` | PARTIAL |
| `SqlxPreparedStatement` | `druid-wrapper/src/sqlx/sqlx_prepared_statement.rs` | `druid-sqlx` | PARTIAL |
| `RbdcConnectionAdapter` | `druid-wrapper/src/rbdc/rbdc_connection_adapter.rs` | `druid-rbdc` | PARTIAL |
| `RbdcDatabaseMetaData` | `druid-wrapper/src/rbdc/rbdc_database_meta_data.rs` | RBDC metadata Adapter | RUST_EXTENSION/PARTIAL |
| `RbdcConnectionFactory` | `druid-wrapper/src/rbdc/rbdc_connection_factory.rs` | `druid-rbdc` | PARTIAL |
| `RbdcPreparedStatement` | `druid-wrapper/src/rbdc/rbdc_prepared_statement.rs` | `druid-rbdc` | PARTIAL |
| `PreparedParameterMaterializer` | `druid-wrapper/src/prepared_parameter_materializer.rs` | SQLx/RBDC 重复资源转换 | RUST_ONLY |
| `PreparedParameterState` | `druid-wrapper/src/prepared_parameter_state.rs` | SQLx/RBDC 重复参数状态 | RUST_ONLY |
| `SqlxBb8ConnectionManager` | `druid-wrapper/src/sqlx/bb8/sqlx_bb8_connection_manager.rs` | `druid-sqlx-bb8` | PARTIAL |
| `SqlxBb8Pool` | `druid-wrapper/src/sqlx/bb8/sqlx_bb8_pool.rs` | `druid-sqlx-bb8` | PARTIAL |
| `SqlxDeadpoolConnectionManager` | `druid-wrapper/src/sqlx/deadpool/sqlx_deadpool_connection_manager.rs` | `druid-sqlx-deadpool` | PARTIAL |
| `SqlxDeadpoolPool` | `druid-wrapper/src/sqlx/deadpool/sqlx_deadpool_pool.rs` | `druid-sqlx-deadpool` | PARTIAL |

#### 平台能力

| Java 平台/生态 | Rust 映射 |
| :--- | :--- |
| `javax.sql.DataSource` | `Pool` provider |
| JDBC raw connection | `PhysicalConnection` |
| c3p0/DBCP/Proxool pool | bb8/deadpool 或 native DruidPool Adapter |
| JNDI `ObjectFactory` | typed config/factory registry |
| MBean | metrics + admin API |
| Java Properties | serde config + aliases |

| Druid 数据源层级 | Rust 对象 |
| :--- | :--- |
| 内置标准实现 | Toasty（不属于 wrapper） |
| direct 扩展 | SQLx、RBDC |
| external pool 扩展 | bb8、deadpool |

### 语义迁移对照表

#### Direct Adapter

| ID | 语义 | 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| WRAP-DIR-001 | 创建/验证/关闭物理连接 | SQLx/RBDC | PARTIAL |
| WRAP-DIR-002 | 参数与行类型映射 | SQLx SQLite 标量及 Prepared 资源；RBDC 严格 SPI | PARTIAL |
| WRAP-DIR-003 | transaction/savepoint | SQLx SQLite/RBDC fixture | PARTIAL |
| WRAP-DIR-004 | PreparedStatement 真实句柄 | SQLx statement 与具名 `RbdcPreparedStatement` | PARTIAL |
| WRAP-DIR-005 | Callable capability | SQLite 显式 unsupported | PARTIAL |
| WRAP-DIR-006 | SQLState/vendor/transient/fatal | 字符串错误 | TODO |
| WRAP-DIR-007 | cancel/timeout/metadata/LOB/stream | SQLx SQLite 与 RBDC SPI 已接 | PARTIAL |
| WRAP-DIR-010 | `PhysicalDatabaseMetaData` Adapter | SQLx SQLite 显式覆盖 160/173 | IMPLEMENTED_UNVERIFIED |
| WRAP-DIR-009 | Prepared generic execute 与 descriptor batch | SQLx SQLite 用真实 statement metadata 区分 query/update | PASS（SQLx SQLite）；PARTIAL（全扩展） |
| WRAP-DIR-008 | `Connection#getWarnings/clearWarnings` | SQLx/RBDC 公开 SPI 不暴露 warning 链 | PASS（SQLite/RBDC SPI 边界） |

#### External Pool Bridge

| ID | 语义 | 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| WRAP-POOL-001 | acquire timeout | bb8/deadpool SQLite | PASS |
| WRAP-POOL-002 | exactly-once return | bb8/deadpool | PASS |
| WRAP-POOL-003 | explicit close 复用 | SQLite | PASS |
| WRAP-POOL-004 | 脏 transaction Drop 置 broken | SQLite | PASS |
| WRAP-POOL-005 | closed pool 拒绝 acquire | deadpool；bb8 矩阵待补 | PARTIAL |
| WRAP-POOL-006 | shutdown 等待 pending lease | 未闭合 | TODO |
| WRAP-POOL-007 | 禁止进入 native idle queue | 类型边界已实现 | PASS |

#### Java wrapper 配置面

| ID | Java 语义 | 当前 | 状态 |
| :--- | :--- | :--- | :--- |
| WRAP-CFG-001 | DBCP/DBCP2 properties aliases | 无统一 factory | TODO |
| WRAP-CFG-002 | c3p0 facade 类型/管理结果 | 无兼容 facade | TODO |
| WRAP-CFG-003 | Proxool property names/defaults | 无 | TODO |
| WRAP-CFG-004 | 时间单位与容量边界 | 各 pool 自有参数 | PARTIAL |
| WRAP-CFG-005 | user/password/delegate properties | 无统一 secret config | TODO |
| WRAP-CFG-006 | JNDI ObjectFactory | 无 | TODO |
| WRAP-CFG-007 | MBean 管理字段 | PoolState 子集 | PARTIAL |

#### 内置/扩展隔离语义

| ID | 语义 | 证据 | 状态 |
| :--- | :--- | :--- | :--- |
| WRAP-BOUND-001 | Toasty 是 `druid` 内置实现 | `druid::toasty` 物理内部模块 | PASS |
| WRAP-BOUND-002 | SQLx/RBDC 是 wrapper 内部可选 direct adapter | 已归入 wrapper 具名内部模块 | PASS |
| WRAP-BOUND-003 | bb8/deadpool 不实现 native factory | 类型与真实 lease 回归 | PASS |
| WRAP-BOUND-004 | 禁止 pool-in-pool | direct adapter 只持 raw connection | PASS |
| WRAP-BOUND-005 | 多数据库 extension capability | SQLite 已证；PG/MySQL 未证 | PARTIAL |

#### 数据库驱动集成账本

| ID | 语义 | 当前证据 | 状态 |
| :--- | :--- | :--- | :--- |
| WRAP-DRV-001 | Toasty SQLite/Turso/MySQL/PostgreSQL feature 依赖闭包 | `cargo check -p druid --all-features --all-targets` | COMPILE_VERIFIED |
| WRAP-DRV-002 | PostgreSQL driver 与 protocol/types 分层 | Toasty manifest | PASS（分类） |
| WRAP-DRV-003 | DynamoDB 不冒充 JDBC SQL connection | `Capability.sql=false` + factory 提前拒绝 | PASS |
| WRAP-DRV-004 | driver-owned Pool 不进入 native factory | 架构约束已定义 | PARTIAL |
| WRAP-DRV-005 | PostgreSQL/MySQL/Turso 真实服务器合同 | 尚未运行 | TODO |
| WRAP-DRV-006 | SQL Server/Oracle/ClickHouse/Firebird/ODBC 候选 | 官方实现已登记，Adapter 未实现 | DISCOVERED |
| WRAP-DRV-007 | DuckDB 原生连接、类型、prepare、事务与统一产品合同 | `duckdb-native` Adapter 和本地真实引擎合同已通过 | ADAPTER_CONTRACT |
