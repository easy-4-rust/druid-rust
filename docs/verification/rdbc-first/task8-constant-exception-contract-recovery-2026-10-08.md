# Types 常量入口与标准异常分类恢复（2026-10-08）

当前结论：标准模块入口与具体 SQL 异常分类已实现，两个正式 RED 均有原始证据；RDBC 和 Druid 指定回归通过。Task6 FIX_REQUIRED、Task8 INCOMPLETE，18 项顶层任务保持开放。

## 真实实现

- types/types.rs 提供全部 39 个 JDBC 4.2 模块常量，每个编号只在模块常量中定义一次；原 Types 关联常量委托同名模块常量，Druid 继续重导出同一 Types。types/mod.rs 公开该标准模块，没有定义新对象。
- SqlExceptionKind 的真实枚举迁至独立 error/sql_exception_kind.rs，规范枚举项为 InvalidAuthorizationSpec；历史 InvalidAuthorization 作为同值关联常量保留，Debug 输出保留原名称。
- SqlException::kind 优先识别 12 个具体标准异常类，随后保留原 recoverable 标志和 SQLState fallback。未篡改 vendor code、消息、SQLState 或具体类名；不宣称任意 vendor 子类的整个继承树已认证。

## RED 与当前验证

生产代码修改前，分类合同实际 exit101：SQLTransientException 的空 SQLState 被错误分类为 General，另一条 vendor fallback 合同通过。公开入口合同实际 exit101/E0432/E0603，缺少规范授权枚举项且 types 模块私有。两组失败原始日志和生产未修改冻结均保留。

| 验证 | 当前真实结果 |
| --- | --- |
| workspace fmt | exit0 |
| 完整 RDBC all-features | exit0，479/0，19 结果块，包含新增两条分类合同、外部消费者及 doctest |
| Druid 核心及指定消费者回归 | exit0，167/0；核心150、新入口2、值合同4、既有标准合同8、外部消费者3 |
| final source purity | exit0，STANDARD_FILES=51；仅限脚本覆盖的源码范围 |
| 严格全 workspace Clippy | 原句柄16070实际exit0，all-targets/all-features/locked/offline/-D warnings，3m41s |
| 原68公开类型探针 | 当前 Cargo JSON 返回的 rlib 支持全量复编译，47PASS/21FAIL，另1项非Rust目标未认证，原探针SHA均未变化 |
| 当前 Rust/Java 同输入差分 | 编译及运行exit0，39常量与24个具体异常/SQLState实例全部一致，差异0；仅限这些值和分类合同 |

[正式 RED](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-constant-exception-red1.summary.json)

[回归机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-constant-exception-regression1.summary.json)

[源码纯净性](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-constant-exception-purity1.summary.json)

## 独立 Java 基线

固定 OpenJDK commit 7373384725418fa645fb4508ec05a869bc7fc3e7 的源码已由现有 Oracle javac 1.8.0_321 编译。此次通过 bootstrap 覆盖加载这些固定类，逐一核对 Types、SQLException 与12个子类的源码 SHA、原编译收据 class SHA 及运行时 resource SHA，14项一致。没有把整个宿主 JDK 声称为固定 OpenJDK 构建。

实际运行取得39个反射常量与12个具体异常在 null/42S02 SQLState 下的24个实例，错误编号17、消息original及可空SQLState均保留。Java classifier 通过真实 instanceof 识别类型；当前 Rust 产物实际调用模块常量及 SqlException::kind，同输入63条记录全部一致。Rust helper未嵌入期望编号或kind名称。未认证全部构造器、vendor子类继承或完整JDBC方法差分。

[Java 运行及来源绑定](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-constant-exception-runtime1.summary.json)

[当前产物全量探针](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes6.summary.json)

[当前同输入差分](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-constant-exception-runtime-differential1.summary.json)

## 兼容性与未完成范围

Types::常量、同一类型身份以及授权分类的值访问、Debug表示保留，不等于全部公开形态兼容。旧 InvalidAuthorization 由枚举 variant 变为关联常量，旧的 variant 专用导入方式必须纳入最终 API diff、breaking allowlist、SemVer 与迁移说明，不能据局部测试判定无破坏性变化。

当前外部编译实际确认旧 `pub use druid_rdbc::error::SqlExceptionKind::InvalidAuthorization` 返回exit1/E0432；规范 `InvalidAuthorizationSpec` 的原始探针通过。迁移到规范枚举项，或使用已验证的限定值访问 `SqlExceptionKind::InvalidAuthorization`；两种形式不能混淆。见[实际破坏性导入证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-legacy-authorization-variant-import1.summary.json)及[迁移指南](../../rdbc/physical-to-standard-migration.md)。

冻结 standard-constant-exception-inputs.json 为1104源码及58控制输入；当前回归逐项变化0。oracle、inventory 与原探针未修改，没有手写LIVE。最终CLI、API/SemVer、完整差分、真实数据库、Task7八义务及P05–P07仍开放。迁移前完整workspace3869/43不能作为当前源码的完整回归结果。

2026-10-08 当前完整workspace回归已启动：原句柄54741，命令cargo test --workspace --all-features --locked --offline --no-fail-fast，冻结standard-constant-exception-inputs.json（1104源码+58控制输入）。日志task8-constant-exception-workspace-runtime1.log可持续读取，live收据仅证明启动，最终须以原句柄和实际Cargo退出码/summary为准；当前未取得完整全仓终态。运行期间不修改冻结源码/控制输入，不重启或取消原进程，不放宽断言、超时或安全策略。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。
