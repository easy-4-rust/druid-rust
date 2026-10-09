# Task7 资源与取消路径只读预检（2026-10-08）

状态：SOURCE_PREFLIGHT_ONLY / RUNTIME_NOT_EXECUTED。正式事实源仍为 `docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md` 的 Task7 八项义务。本报告不是新规格、第二份实施计划或 Task7 开始/完成许可。Task6 当前仍FIX_REQUIRED，完整C2正在运行；本轮不修改冻结Rust输入、不启动第二个Cargo。

## 当前对象的真实位置

CodeGraph当前磁盘源码确认：

| 对象 | 当前源码 | 已存在行为与边界 |
| --- | --- | --- |
| ResourceOperations | crates/druid-rdbc/src/resource/rdbc_resource_capabilities.rs | READ/WRITE/SEARCH/TRUNCATE/STREAM/RANGE/TYPE_MAP/RESULT_SET/FREE；bitflags存在不证明所有声明行为已验收 |
| ResourceHandleFactory | crates/druid-rdbc/src/resource/rdbc_resource_factory.rs | 六类detached handle构造与context校验；context-bearing constructors为crate私有，并显式标注预留Task7 |
| ResourceLifecycle | crates/druid-rdbc/src/resource/rdbc_resource_context.rs | shared identity、capability gate、Open/Releasing/Freed/Invalid状态、free幂等和错误回退 |
| ResourceContext | crates/druid/src/core/resource_context.rs | Druid私有owner、失败/释放/abandon通知及异步release_with |
| ResourceFactory/ResourceOwner | crates/druid/src/core/resource_factory.rs、resource_owner.rs | Druid私有owned context与owner回调，不能当作标准crate的产品依赖 |
| RdbcSqlXml | crates/druid-rdbc/src/resource/sql_xml.rs | 读写方法经过能力和Open gate；本handle未见独立SQLXML读写阶段状态，是否由各access实现保证需实际合同测试 |
| ConnectionCapabilities | crates/druid-rdbc/src/extensions/connection_capabilities.rs | 五项bool默认false；声明存在不证明true对应行为 |

初次猜测的 `resource/resource_operations.rs` 未被CodeGraph匹配。实际对象位于旧文件名 `rdbc_resource_capabilities.rs`，不得以猜测路径建立待实施清单。

## 标准资源 free 的静态取消窗口

`ResourceLifecycle::free` 当前98–143行：先CAS Open→Releasing；其他调用在Releasing等待Notify；随后直接 `access.free().await`。驱动返回Err才回退Open并通知，返回Ok才写Freed并通知。方法内未见drop guard或受监管任务接管该await。

**源码推断，尚无本轮runtime证明：** 若赢得CAS的调用future在驱动await未完成时被丢弃，状态可能停在Releasing，后继free调用可能继续等待。需要在真实public标准handle链上使用可控driver释放屏障，确认abort后共享alias能够完成释放/观察明确结果，不能用Python模型或复制实现证明产品行为。

## Druid owner 的第二个静态取消窗口

`ResourceContext::release_with` 当前89–139行：Open→Releasing后直接 `release().await`；driver成功后还会 `owner.resource_released(...).await`，最后才写Freed及通知。Drop只在Open状态通知abandon，见173–179行。

**源码推断，尚无本轮runtime证明：** 取消可能发生于driver释放前，也可能发生于driver成功、owner通知未完成之间。第二种窗口若只简单回退Open，可能重放driver release，违背driver release与owner notification各自exactly-once要求。后续实现需保持既有单一标准和Druid私有owner边界，不能通过重新引入Physical/public owner port规避。

现有三项ResourceContext测试覆盖并发release通知一次、Notify无丢失唤醒、invalidate/abandon政策。CodeGraph源码显示它们没有abort赢者或abort owner回调的场景；这些已存在测试不应被报告为取消保护证明。

## Task7 八项义务的预检结论

| 正式义务 | 本轮发现 | 未取得的必要证据 |
| --- | --- | --- |
| RESOURCE_OPERATIONS_AND_FACTORY | 类型、detached factory、context校验存在 | 实际标准operation/factory shared contracts，以及产品owner接线 |
| RESOURCE_TRAIT_LIFECYCLE | Open/能力门禁存在；SQLXML状态需区分access实现 | 1-based边界、free后访问、SQLXML状态矩阵 |
| RDBC_RESULT_SET_FORWARDING | 本轮未完整核验 | 独立forwarding身份与实际消费者链，不能用RowSet替代 |
| CAPABILITY_TRUTHFULNESS | 五bool快照存在且默认false | 每个capability=true的实际driver/pooled行为矩阵 |
| DRUID_OWNER_RELEASE_EXACTLY_ONCE | 正常并发通知测试存在，driver后还有owner await | driver与owner各自唯一计数、失败/重试及取消窗口 |
| DRUID_CANCELLATION_HARDENING | 上述两个未接管await窗口可定位 | driver/owner两个屏障处abort、supervisor drain、容量和终态证明 |
| SQLX_BUFFERED_RENAME | 前次当前源码确认SqlxStreamingResultSet保存Vec<Row> | rename和全部消费者/API路径一致性 |
| ASYNC_RESULT_SET_STREAMING | 不能将Vec游标的async next视为真实stream | 有界首行、背压、取消、driver生命周期与Stream实际证据 |

八项全部保留TASK7_PENDING。本轮不产生PASS数量或TDD RED记录。后三项以及各driver接线还需在正式前置条件满足后继续CodeGraph与运行合同核验。

## 与当前C2的关系

原runtime4/session56915/Python43372真实推进：02:02:46外部消费者测试经自身nested Cargo46240运行；02:04:16已进入wrapper all-features sqlx_bb8_pool_test/Cargo47928，02:04:55其rustc30640存活。子进程推进不是完整C2聚合通过证明，继续同一句柄。

本报告没有改动Rust/Cargo/proto/SQL、原1283/479分母或正式验证规则。compiler/API/SemVer/MSRV/features/网络与最终审查、全部18顶层任务仍开放。
