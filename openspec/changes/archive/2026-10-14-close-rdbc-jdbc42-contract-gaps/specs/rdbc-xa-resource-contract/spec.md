# 标准 XA 资源

## Purpose

使事务管理器通过标准 XA 连接取得可操作的资源，完成事务分支控制和崩溃恢复，而不是只读取资源名称；同时让不支持 XA 的后端明确报告能力边界和结构化错误。

## ADDED Requirements

### Requirement: XA-01 XA 连接提供可操作资源

支持 XA 的连接 SHALL 返回具有 start、end、prepare、commit、rollback、forget、recover、资源管理器比较和事务超时操作的标准资源；资源身份 SHALL 关联原物理 owner，名称不能替代操作接口。

#### Scenario: 标准事务管理器获取资源
- **WHEN** 事务管理器只依赖 RDBC 标准接口获取 XA 连接及其资源
- **THEN** 能调用资源操作而无需导入 Druid 产品实现类型

#### Scenario: 后端不支持 XA
- **WHEN** 对不具备 XA 的数据源请求 XA 连接或资源
- **THEN** 返回 feature-not-supported/0A000，不能以本地事务或空资源伪装 XA 成功

### Requirement: XA-02 分支和协议结果完整传递

XA 操作 SHALL 保留 Xid 的格式标识、全局事务标识和分支标识，以及协议 flags、一阶段提交标志、prepare 只读结果和标准 XA 错误码；非法输入 SHALL 在副作用前拒绝。

协议 flag 和 prepare 返回值 SHALL 与冻结的 JDK 8 `javax.transaction.xa` 常量数值一致：`TMNOFLAGS=0x00000000`、`TMJOIN=0x00200000`、`TMRESUME=0x08000000`、`TMSTARTRSCAN=0x01000000`、`TMENDRSCAN=0x00800000`、`TMSUCCESS=0x04000000`、`TMFAIL=0x20000000`、`TMSUSPEND=0x02000000`、`TMONEPHASE=0x40000000`、`XA_OK=0`、`XA_RDONLY=3`。实现不得暴露非标准的同名替代值；来源及校验结果记录在 XA 证据中。

#### Scenario: 两阶段与只读投票
- **WHEN** 分支分别投票 prepared 和 read-only
- **THEN** prepared 分支可执行第二阶段提交，read-only 结果被显式识别且不被错误转换为普通 prepared

#### Scenario: 无效分支或 flags
- **WHEN** 提交未知分支或传入不合法的协议组合
- **THEN** 返回对应 XA 错误，不丢失底层原因且不执行无关分支操作

#### Scenario: JDK 8 协议常量兼容
- **WHEN** 外部事务管理器使用冻结 JDK 8 XA 常量调用 start、end、commit 或 recover
- **THEN** 每个常量的数值与 JDK 8 来源一致，协议操作解释其标准含义，并由常量单测和外部消费者测试共同验证

#### Scenario: 非法操作不改变分支
- **WHEN** 分支状态不允许某操作、Xid 未知或 flag 组合非法
- **THEN** 操作返回对应协议错误，分支状态、关联关系及其他 Xid 均保持原样

### Requirement: XA-03 恢复扫描与资源管理器身份

资源 SHALL 保留恢复扫描的开始、继续和结束语义，返回完整 Xid，且只返回可恢复的分支（prepared 或启发式完成）；扫描分页期间 SHALL 使用稳定的扫描快照，不能因分页重复、遗漏或伪造分支。

`TMSTARTRSCAN` SHALL 开始新扫描，`TMNOFLAGS` SHALL 仅继续活动扫描，`TMENDRSCAN` SHALL 结束活动扫描；标准允许的 `TMSTARTRSCAN | TMENDRSCAN` SHALL 表示一次性扫描。非法 flag 组合或不合法的扫描阶段 SHALL 返回 XA 协议错误，且不得改变分支状态或破坏现存扫描。实现可以返回空页表示扫描完成，但必须记录其分页与完成语义。

同资源管理器比较 SHALL 依据稳定、不可伪造的资源管理器身份，不能依据显示名称、连接字符串或偶然相同的配置文本。无法证明身份相同的两个不同资源句柄 SHALL 保守地返回 false；同一资源对象可依据对象身份判为 true。多个不同连接句柄只有持有同一身份凭证时才可返回 true。

#### Scenario: 重建连接后恢复 prepared 分支
- **WHEN** 一个分支已 prepare、连接重建后事务管理器进行恢复扫描
- **THEN** 返回可恢复分支并允许按协议提交或回滚，扫描不会返回伪造分支

#### Scenario: 分页恢复只返回可恢复分支
- **WHEN** 恢复扫描包含多个状态不同的分支，且结果超过单页容量
- **THEN** 各页只返回 prepared 或启发式完成分支，完整 Xid 无损，页间无重复或遗漏，且扫描使用开始时确定的快照

#### Scenario: 恢复扫描 flags 阶段约束
- **WHEN** 事务管理器依次开始、继续和结束扫描，或执行标准的一次性开始并结束扫描
- **THEN** 扫描状态按 flags 转换；无活动扫描时继续/结束或传入非法组合均返回 XA 错误，不修改事务分支或其他有效扫描

#### Scenario: 名称相同但资源管理器不同
- **WHEN** 两个不同资源管理器具有相同诊断名称
- **THEN** 同资源管理器比较返回 false

#### Scenario: 多个连接句柄属于同一资源管理器
- **WHEN** 两个连接句柄由同一资源管理器身份凭证创建
- **THEN** 同资源管理器比较返回 true；只提供相同名称或连接属性而无法证明身份时返回 false

### Requirement: XA-04 生命周期和超时受 owner 管理

XA 资源 SHALL 不绕过原连接 owner 的关闭和错误处置；默认/自定义事务超时的设置结果与读取 SHALL 保留驱动语义，不支持的超时设置不得伪造成功。

#### Scenario: owner 已关闭
- **WHEN** 获取资源后关闭其物理 owner，再调用资源
- **THEN** 返回明确的失效或 XA 资源错误，不访问已释放连接

#### Scenario: 驱动拒绝超时设置
- **WHEN** 驱动不支持所请求的事务超时设置
- **THEN** 返回其拒绝结果，后续读取不能声称新超时已生效

### Requirement: XA-05 JDBC Agent XA 客户端闭环

JDBC Agent 的 Rust 客户端 SHALL 仅在调用方显式配置 XA 数据源时请求 XA session，并 SHALL 将标准 RDBC `XaResource` 操作映射到已建立的同一 Agent session。SQL 连接句柄与 XA owner SHALL 分离：关闭逻辑连接不得提前关闭 XA owner；关闭 owner 后，连接及资源操作均须确定地失败。普通 JDBC Agent session SHALL 不声明 XA 能力。客户端 SHALL 无损保留 Xid、flags、prepare 投票和结构化 XA 错误。

启用 XA 的新增配置和协商 SHALL 保持既有普通 JDBC Agent 连接载荷及运行时握手行为不变；普通连接不得被迫发送 XA 属性、能力请求或额外握手字段。XA 能力只由显式 XA 配置建立的 session 响应协商确认，不能由 URL、驱动名称或普通连接属性推断。

XA 配置 SHALL 提供明确的 XADataSource 类名和属性映射；配置解析、类加载、实例化、属性设置或握手任一步失败时，客户端 SHALL 返回带来源的错误并关闭已创建资源，不得回退到 DriverManager/普通 JDBC Connection。XA owner、逻辑连接和 XA RPC SHALL 共享同一个 session 串行化边界；逻辑句柄关闭后旧代次必须立即失效，新句柄不得被旧代次的迟到操作触达。底层传输进入结果不确定或不可恢复状态时，owner SHALL 被隔离并拒绝再次借出。

当 XA owner 同时作为标准 `PooledConnection` 暴露时，逻辑连接关闭 SHALL 通过该 owner 的连接事件注册表通知一次，事件源必须是实际且稳定的 XA owner。池归还逻辑连接不得等同于关闭 XA owner；在 XA 生命周期仍有效时，`XaResource` 必须继续可用。owner 关闭后，所有逻辑租约与资源均失效。传输错误触发隔离时，即使原始 `RdbcError` 不含 `SqlException`，也必须发出一次足以供外部池判定 discard 的连接错误事件，并保留安全的传输诊断上下文；调用方仍收到原始主错误，清理错误不得覆盖它。事件回调不得在持有 session、池或事件注册表锁时运行。

本要求只要求 JDBC Agent 与已验证支持恢复的后端闭环，不要求 SQLx、Toasty 或其他适配器普遍支持 XA。后端未配置 XA 或能力未协商时，必须 fail closed 并返回精确的不支持错误。

#### Scenario: 显式 XA 配置创建客户端资源
- **WHEN** 调用方配置 XADataSource 类名及属性，并通过 JDBC Agent 建立 session
- **THEN** RDBC 客户端可从该 session 获取标准 `XaResource`，并使用同一底层 XA owner 执行 SQL 与 XA 分支操作

#### Scenario: 普通 session 不被提升为 XA
- **WHEN** 调用方使用普通 JDBC URL/属性建立 Agent session 后请求 XA 能力
- **THEN** 客户端返回精确的不支持错误，不伪造 XA 资源或以本地事务替代

#### Scenario: 普通连接载荷和握手保持兼容
- **WHEN** 调用方使用既有普通 JDBC Agent 配置建立连接
- **THEN** 发出的普通 `open_session` 载荷及运行时握手与变更前一致，不包含 XA 配置或能力请求，原有 SQL 行为保持可用

#### Scenario: XA owner 与逻辑连接分别关闭
- **WHEN** 事务管理器关闭逻辑 SQL 连接但仍持有 XA owner
- **THEN** 同一 session 的合法 XA 操作仍可继续；当 XA owner 关闭后，所有关联句柄均失效且不能触达已关闭资源

#### Scenario: XA 逻辑关闭通知且保留 owner
- **WHEN** 池化 XA 连接的逻辑 SQL 句柄被关闭并归还
- **THEN** 标准连接关闭事件由原 XA owner 发出且只通知一次；XA owner 和 `XaResource` 仍可用于后续合法事务操作，只有显式关闭 owner 才释放 session

#### Scenario: XA owner 传输故障通知
- **WHEN** SQL 或 XA RPC 返回致命错误或不确定传输状态
- **THEN** owner 被隔离且不再借出；即使原始错误不是 `SqlException`，标准连接错误事件仍携带可供外部池 discard 的原因及安全诊断上下文，调用方收到的仍是原始主错误；监听器重入或失败不会阻断 session 清理，也不会让错误连接重新进入池

#### Scenario: 显式 XA 配置失败时不降级
- **WHEN** XADataSource 类名无效、属性配置失败、Agent 拒绝 XA 协商或握手返回未协商 XA
- **THEN** 创建操作返回可诊断错误并清理已创建资源，不打开普通连接作为替代；若 session 未协商 XA，标准能力错误保持 SQLState `0A000`

#### Scenario: 逻辑代次隔离
- **WHEN** 旧逻辑连接关闭并从同一 XA owner 获取新逻辑连接后，旧句柄再次执行 SQL 或迟到的关闭请求到达
- **THEN** 旧句柄被确定地拒绝，迟到操作不关闭、不修改新逻辑连接或 XA owner

#### Scenario: owner 关闭及传输故障处置
- **WHEN** owner 被重复关闭，或一次 SQL/XA 请求使传输状态无法确认
- **THEN** 重复关闭幂等；不确定 session 被标记不可复用，所有关联逻辑句柄和资源拒绝后续操作，原始协议/SQL 错误及清理错误可诊断且互不覆盖

#### Scenario: 客户端跨 session 恢复
- **WHEN** 客户端通过 Agent prepare 一个事务分支、关闭原 session，再建立新的 XA session 并恢复
- **THEN** recover 返回完整 Xid，后续 commit/rollback 对真实数据库产生可查询的终态，协议错误码和投票保持原值

### Requirement: XA-06 对外集成文档和兼容性审查

仓库 SHALL 提供面向外部应用和事务管理器维护者的 XA 集成说明，描述 feature 启用、明确的 XADataSource 类名与属性配置、`XaDataSource → XaConnection → XaResource` 获取路径、逻辑连接租约与物理 XA owner 的关闭边界、跨 session 恢复步骤、错误处置和凭据脱敏。示例 SHALL 区分普通 JDBC Agent 连接与 XA session；普通连接不得被暗示可升级为 XA。文档中的 Rust 调用示例 SHALL 由只依赖公开 RDBC API 的最小消费者编译验证；关键 XA 生命周期与恢复示例 SHALL 有已运行的合同或实库证据支持。

XA API 变更 SHALL 逐项记录公开类型、方法、feature gate、源兼容性及 SemVer 影响。不能以类型存在、旧 API 快照或编译成功替代对本次真实 API 差异的审查；若无已发布稳定版本，记录该事实和发布前的版本策略建议，不得伪称已作版本发布。

#### Scenario: 外部事务管理器使用显式 XA 数据源
- **WHEN** 外部消费者启用 `jdbc-agent` 并显式配置 XADataSource 类名和属性
- **THEN** 消费者仅通过公开 `druid-rdbc` 合同取得 XA owner/resource，并能按接口执行 start、SQL、end、prepare 与第二阶段操作，不依赖 wrapper 私有类型

#### Scenario: 逻辑租约关闭与 XA owner 关闭
- **WHEN** 事务管理器关闭逻辑 SQL 连接而保留 XA owner
- **THEN** 文档明确该操作只归还/失效逻辑租约，owner 上的 XAResource 仍有效；关闭物理 XA owner 后所有关联句柄失效

#### Scenario: 恢复与不确定故障处置
- **WHEN** 事务管理器在新 XA session 恢复 prepared Xid，或遇到 XA/传输错误
- **THEN** 文档给出 recover→核验完整 Xid→commit/rollback 的操作顺序、支持恢复的后端前提、错误码/SQLState 处理及 owner 隔离规则；证据明确标注实际验证的后端和版本

#### Scenario: 普通模式迁移与公开 API/SemVer
- **WHEN** 普通 Agent 用户阅读迁移说明或维护者评审 XA API
- **THEN** 普通连接配置和握手保持原样，XA 能力必须通过显式配置创建；新增、弃用、破坏性 API 与 feature gate 均列入差异记录，并由外部消费者编译结果佐证
