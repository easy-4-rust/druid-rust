# rdbc-common-data-source Specification

## Purpose
补齐普通、池化和 XA 数据源共用的日志及登录超时合同，使外部调用方能通过统一接口配置诊断、识别超时和获取原始错误，同时保持数据源之间以及与全局驱动管理器之间的隔离。

## Requirements

### Requirement: CD-01 登录超时支持默认值与失败结果

数据源 SHALL 提供 `try_login_timeout() -> Result<Option<Duration>, RdbcError>` 可失败读取合同，并保留已有 `set_login_timeout` 的可失败写入合同。读取或写入的底层错误 SHALL 原样返回，不能转换为默认值、`None` 或成功。

旧 `login_timeout() -> Option<Duration>` SHALL 保留为 deprecated 迁移入口；其默认适配 SHALL 返回 `Ok(old_value)`，确保既有外部实现保持源码兼容。能观察底层读取失败的实现 SHALL 覆盖 `try_login_timeout`，新消费者 SHALL 使用可失败入口。

`None` SHALL 表示实现没有显式配置并沿用系统/驱动默认期限；`Some(Duration::ZERO)` SHALL 表示明确不限制登录等待；正时长 SHALL 按原精度保留。Java 有符号秒数转换为 Rust `Duration` 时，负值和超出来源表示范围的输入 SHALL 明确返回错误，不得截断或饱和为默认值。

#### Scenario: 默认与零值
- **WHEN** 创建未配置数据源或显式设置登录超时为零
- **THEN** 可失败 getter 分别返回 `Ok(None)` 和 `Ok(Some(Duration::ZERO))`；前者沿用系统/驱动默认，后者不限制登录等待

#### Scenario: 查询或设置失败
- **WHEN** 底层数据源在读取或设置登录超时时返回 SQL 错误
- **THEN** 调用方从 getter/setter 收到原错误载荷，不能将其转换为默认值、`None` 或配置成功

#### Scenario: 旧实现源码兼容迁移
- **WHEN** 既有外部实现只覆盖旧 `login_timeout`，调用方改用 `try_login_timeout`
- **THEN** 实现继续编译，旧值通过 `Ok` 保留；新实现可覆盖可失败入口提供读取错误

#### Scenario: 时长范围与精度
- **WHEN** 适配器将 Java 秒数转换为 Rust 时长，或公开 API 往返最大时长、纳秒精度
- **THEN** 所有可表示输入精确保留，非法负数或来源范围溢出返回明确错误，不截断、不饱和且不伪装成默认值

### Requirement: CD-02 数据源日志可配置且相互隔离

普通、池化和 XA 数据源 SHALL 通过公共合同提供中立的日志接收对象获取和设置能力。日志接收对象初始为空；设置接收对象后，数据源及其产生对象的诊断 SHALL 使用该数据源配置；设置为空值 SHALL 禁用后续写入。日志接收对象按数据源实例隔离，不得隐式继承、修改或污染全局 `DriverManager` 或其他数据源的配置。日志 API 不得要求调用方依赖具体日志框架类型。

#### Scenario: 两个数据源与全局日志独立
- **WHEN** 分别为数据源 A、B 和 DriverManager 设置不同日志接收对象
- **THEN** A 的诊断仅进入 A 的接收对象，B 与全局接收对象不收到 A 的诊断

#### Scenario: 日志接收对象初始为空并可设置
- **WHEN** 创建普通、池化或 XA 数据源，随后分别读取并设置日志接收对象
- **THEN** 初始读取均为空；设置后从同一数据源入口读取到相同接收对象，并可由该数据源诊断路径调用

#### Scenario: 禁用日志
- **WHEN** 将已配置的数据源日志接收对象设为空
- **THEN** 随后的数据源诊断不再写入原接收对象

### Requirement: CD-03 父日志命名空间保持显式能力

数据源 SHALL 提供父日志命名空间查询；Rust 映射 SHALL 不依赖 Java Logger 对象，支持时返回稳定命名空间，不支持时返回 feature-not-supported/0A000。

#### Scenario: 三类数据源共享合同
- **WHEN** 调用方通过普通、池化或 XA 数据源的公共接口查询日志配置
- **THEN** 均能访问相同公共合同，成功和不支持结果保持一致的类型语义

#### Scenario: 父命名空间能力缺失
- **WHEN** 数据源实现没有可报告的父日志命名空间
- **THEN** 查询以 SQLState `0A000` 明确失败；调用方不会把空值或伪造名称当成受支持结果

### Requirement: CD-04 诊断不泄露连接凭据

公共配置和连接诊断 SHALL 对密码、访问令牌及 URL/属性中的敏感字段做脱敏，同时保留安全的错误类别和诊断上下文。日志接收器发生 I/O 故障时，调用方 SHALL 能观察该日志故障；若同时存在连接/配置 SQL 错误，原 SQL 错误 SHALL 保持主错误且其 SQLState、消息和来源不被日志故障覆盖。日志故障不得改变连接结果或已生效的配置状态。

#### Scenario: URL 与属性含凭据
- **WHEN** 连接或配置失败且输入中包含用户名密码、token 或敏感查询参数
- **THEN** 日志不包含秘密值，仍保留可定位问题的错误类型与安全上下文

#### Scenario: 日志 I/O 故障与连接错误同时发生
- **WHEN** 连接失败并返回 SQL 错误，同时日志接收器也发生 I/O 故障
- **THEN** 原 SQL 错误仍是调用方看到的主错误，日志故障可单独诊断，连接失败不会被伪装成成功

#### Scenario: 日志 I/O 故障不改变已应用配置
- **WHEN** 配置操作已成功应用，但随后记录诊断时日志接收器写入失败
- **THEN** 配置读取仍反映已应用值，日志故障不会回滚或伪报配置操作失败

### Requirement: CD-05 CommonDataSource has one standard trait identity

`druid-rdbc::data_source::CommonDataSource` SHALL be the only `CommonDataSource` trait identity. `druid::sql::CommonDataSource` SHALL remain an exact named re-export for source-path migration and SHALL NOT define another trait or independent method defaults. Druid and wrapper data sources SHALL implement the canonical contract once. The approved pre-release migration replaces the old Druid integer-seconds getter and setter with the standard `try_login_timeout`/`Duration` interface; callers must observe standard unsupported and error results rather than a fabricated numeric default.

#### Scenario: Both public import paths identify one contract
- **WHEN** an application imports `CommonDataSource` from either `druid::sql` or `druid_rdbc::data_source`
- **THEN** both paths expose the same trait identity and method semantics without duplicate implementations

#### Scenario: Dynamic data source has no configured timeout
- **WHEN** a dynamic data source has no explicit login timeout or parent logger
- **THEN** `try_login_timeout` returns `Ok(None)` and `get_parent_logger` returns feature-not-supported with SQLState `0A000`
