# Spec Delta

## Purpose

为跨资源事务提供可持久化、可恢复且不会重复提交的两阶段提交协调，确保进程崩溃、部分参与者完成和结果不确定时仍能形成可解释的终态。

## ADDED Requirements

### Requirement: 决策与参与者结果必须持久化

协调器 SHALL 在外部副作用前按协议顺序持久化事务、参与者、准备投票、提交/回滚决策及完成结果，记录键采用 `2pc:tx:{id}` 形式并支持幂等重放；进程内状态或资源 recover 不得替代协调日志。

#### Scenario: 协调决策前进程崩溃
- **WHEN** 在 prepare 后、commit 决策落盘前发生崩溃
- **THEN** 重启可重建事务与参与者状态，不将未决事务误报为已提交或已回滚

### Requirement: 重放必须幂等且不重复副作用

恢复重放 SHALL 识别已记录的决策和参与者完成结果，已完成操作不得重复提交、重复回滚或重复发送不可逆命令。

#### Scenario: 同一事务日志被重复重放
- **WHEN** 恢复流程对同一 `2pc:tx:{id}` 日志执行多次 replay
- **THEN** 外部效果至多发生一次，终态与首次成功恢复一致

### Requirement: 部分参与者结果保持显式可区分

协调器 SHALL 分别保存每个参与者的 prepared、committed、aborted、unknown 和 Mixed 状态；网络错误或无法确认结果 SHALL 记录为 unknown，不得等同于未提交。

#### Scenario: 提交过程中部分参与者失联
- **WHEN** 一部分参与者已提交而其他参与者结果无法确认
- **THEN** 事务保留 Mixed/unknown 及逐参与者证据，不自动重试已提交参与者，也不伪称全局提交成功

### Requirement: Mixed 状态禁止以再次提交探测

Mixed 或 unknown 状态 SHALL 仅允许只读核验、重放既有决策、人工或已批准策略裁决；系统不得通过再次 commit 判断参与者是否已提交。

#### Scenario: 运维或恢复流程请求重试提交
- **WHEN** 事务处于 Mixed 或参与者提交结果未知
- **THEN** 系统拒绝再次 commit，并返回可诊断的未决状态和所需核验步骤

### Requirement: 崩溃恢复矩阵必须验证真实终态

协调器 SHALL 在 prepare 前、prepare 后、决策前、部分 commit、恢复中和重复恢复等崩溃窗口进行故障注入，验证日志、参与者与数据库终态以及清理行为。

#### Scenario: 进程在部分参与者提交后重启
- **WHEN** 恢复服务读取持久化日志并连接原参与者
- **THEN** 已完成分支不重复提交，未决分支按已落盘决策和恢复规则处理，测试保留各参与者最终状态

### Requirement: XA 资源能力与协调器能力分别声明

现有 XA 分支状态机和资源恢复接口 SHALL 与 durable coordinator 分开验收；仅实现 XA 资源操作不得被描述为已具备持久化协调器。

#### Scenario: 仅有进程内 XA 状态和资源 recover
- **WHEN** 实现支持 start/end/prepare/commit/recover 但没有持久化协调日志与重放证据
- **THEN** 只能声明 XA 资源合同，durable 2PC 仍为未完成
