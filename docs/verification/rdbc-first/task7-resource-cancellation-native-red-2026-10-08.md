# Task7 资源取消与线程边界实际 RED（2026-10-08）

Task7 尚未完成，八项正式义务不变。本报告将先前源码预检升级为两个当前产物的实际反例，不修改生产源码、正式测试或冻结控制输入。

## 取消释放实际失败

通过当前 Cargo JSON 产物的公开 ResourceHandleFactory::blob 创建资源并克隆句柄。受控驱动在 free 进入时通知测试，然后等待完成屏障。测试取消首个调用者，打开驱动完成屏障，再从另一句柄调用 free。

rustc 编译实际 exit0；运行实际 exit101。原始输出：

```text
aborted=true driver_calls=1 driver_finished=false alias_freed=false successor_timeout=true read_rejected=true
cancelled free must not strand the shared resource in Releasing
```

后继 free 在250ms内未完成。这是实际公开 Blob/factory/clone/free 经过生产 ResourceLifecycle 的运行证据，不是复制状态机模型。不能据有限观察宣称实验已证明无限等待。驱动未完成、资源未标记释放、读取被拒绝；源码在 Open→Releasing 后直接 await access.free，无取消完成路径，与观察相符。

[机器收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-resource-cancellation-native-red1.summary.json)

[原始运行日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-resource-cancellation-native-red1.log)

## Send 与 Sync 边界实际失败

正式规格 §7.2 要求可变资源 Send+'static；Sync 仅供独立只读视图。旧 RdbcResourceAccess 强制 Debug+Send+Sync。

同一个包含 Cell<usize> 的 LocalResource：独立 Send+'static 编译控制实际 exit0；增加真实 RdbcResourceAccess impl 后实际 exit1/E0277，唯一错误定位旧 trait 的 Sync bound。两次编译使用同一当前 RDBC 产物，未在探针中定义替代 trait。

因此不能把旧 access trait 简单重导出为 canonical 资源 trait 并宣称完成：其线程边界仍偏离正式规格。标准资源迁移必须同时处理独占可变访问、实际 adapter 实现和 Druid owner 包装。

[编译控制与反例收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-resource-send-bound1.summary.json)

## 产物、冻结与下一步

两组探针使用真实 libdruid_rdbc.rlib，SHA256：559a44986aef0088a894a68833aa6a1d1a6e8a07bca27e970c9c6dbbe7f6f670；来源沿用当前 Cargo JSON artifact 收据。原始helper、日志、命令及收据保留，日志/源码/收据逐字节镜像至仓库证据目录。

本次逐项核对 standard-constant-exception-inputs.json：1104源码和58控制输入变化0。全workspace原句柄54741已确认仍在运行，不能修改这些冻结输入、取消或重启运行。上述探针均在F盘独立编译，不写入workspace target或生产树。

全仓终态后，将失败行为接入计划指定的正式 resource_standard_contract_test / resource_owner_cancellation_test，并完成统一标准资源、owner通知与驱动释放分别恰好一次、取消监管和失败路径测试。单个Blob反例不认证其他资源类型、owner通知、SQLXML或八义务全部完成。Task6 FIX_REQUIRED、Task8 INCOMPLETE、全部18顶层任务继续开放。

## Druid 私有 owner 两阶段取消实际 RED

独立helper通过 Rust path 属性直接包含当前生产树六个未修改的私有模块（resource_context/owner/id/kind/state/runtime_capabilities），而非复制状态机或修改可见性。编译实际exit0；两个窗口分别运行exit101：

```text
window=driver state=Releasing driver_calls=1 owner_calls=0 owner_completed=0 successor_timeout=true
window=owner state=Releasing driver_calls=1 owner_calls=1 owner_completed=0 successor_timeout=true
```

driver窗口在驱动释放屏障取消首调用；owner窗口在驱动成功、owner回调已进入但尚未完成时取消。打开完成屏障后，后继调用均在250ms内未完成。后者证明修复需要分别跟踪驱动完成和owner通知完成，不能将状态简单恢复Open并重跑驱动释放。

此为当前私有生产源的独立编译运行证据，未使用Druid已编译公共集成路径，未认证真实池owner注册/归还/关闭，也未执行完整Druid测试；这项范围限制必须保留。六个源SHA、编译命令、两个原始日志及helper均记录在收据。

[私有生产源两窗口实际收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-druid-owner-source-red1.summary.json)

## 可执行回归草稿基线

新增独立Rust测试草稿直接包含相同六个生产模块，运行六个新增合同测试及生产源自带三个单元测试。实际编译exit0，测试exit101：7PASS/2FAIL/0ignored。两项失败精确为首调用者在驱动释放阶段取消、首调用者在owner通知阶段取消。

新增四项通过控制覆盖：并发成功只释放/通知一次；驱动失败后可重试且失败时不提前通知owner；owner返回错误不重放已成功驱动释放；取消等待者不取消获胜释放。原生产三个测试同样通过。helper与真实原始日志已保存，尚未接入Cargo正式测试，不能记为Task7 GREEN。

[9项实际测试收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-druid-release-contract-draft1.summary.json)

[真实测试日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-druid-release-contract-draft1.log)

本轮结束前再次核对1104源码/58控制冻结：变化0；六个实际包含的生产源SHA与初始反例一致。全仓原54741仍待终态，未修改生产、正式测试或控制输入。

## 标准资源接入范围核查

CodeGraph当前源码确认：规范Array/Blob仍为具体Rdbc句柄别名；Connection::create_blob/create_clob/create_n_clob与ExternalConnectionLease返回并转发具体句柄。六个access实现不能只换名称，否则仍保留Sync约束与access/context第二套端口。标准trait、兼容句柄实现内核、Connection返回链及Druid owner包装必须协同迁移，不能仅修改公开alias使探针变绿。

对crates内精确impl语句的候选枚举得到66处：Array9、Blob11、Clob17、NClob10、Ref10、SqlXml9。63处位于tests路径，另外3处位于src/lease_complete_tests；CodeGraph确认其父模块第98行cfg(test)，所以这些候选均为测试实现。此为词法候选清单，不认证macro/generic/别名隐藏实现，也不能由未检出直接推断所有数据库资源都不存在。它界定了现有兼容测试迁移范围，不能作为真实数据库资源认证证据。

[源绑定实现候选清单](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-resource-access-migration-inventory1.summary.json)

Wrapper现有标准解包enum未列这些资源；Java资源是否继承Wrapper应按固定Java来源分别判断，不能为了接入而凭空扩大接口继承。
