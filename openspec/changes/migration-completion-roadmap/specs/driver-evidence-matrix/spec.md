# Spec Delta

## Purpose

让数据库驱动支持状态由真实运行证据决定，区分目录声明、实验能力、已验证支持和认证支持，防止把协议兼容或产品名称当成完整互通。

## ADDED Requirements

### Requirement: 支持状态必须由证据晋升

驱动 profile SHALL 保持 declared 或 experimental，直到存在对应行为证据；verified/certified SHALL 关联具体产品、版本、驱动、平台和可复核运行产物。

#### Scenario: 仅有目录、注册表或连接工厂
- **WHEN** profile 有名称、URL 映射或工厂实现但没有真实运行结果
- **THEN** 不计入公开支持数量，也不显示为已验证

### Requirement: 矩阵覆盖正向和负向合同

每个纳入验收的 profile SHALL 验证连接、查询、更新、预编译、元数据、类型、NULL、批处理、事务、保存点、取消、关闭及异常分拣；不支持能力 SHALL 返回精确错误而非伪造成功。

#### Scenario: 某能力不受驱动支持
- **WHEN** 驱动明确不支持事务、XA、流式读取或特定元数据
- **THEN** 矩阵记录负能力及错误证据，不将其写成能力缺口已关闭，也不以默认成功蒙混

### Requirement: 真实宿主结果不得被忽略替代

真实数据库、进程、容器或网络验证 SHALL 有实际运行结果；未配置环境、被忽略或超时的测试 SHALL 标为 NOT_RUN，不能作为通过证据。

#### Scenario: 真实 XA 宿主未运行
- **WHEN** JDBC Agent、驱动或数据库环境缺失导致 XA 测试 ignored
- **THEN** 对应恢复与事务能力保持未验证，不计通过数量

### Requirement: 故障与恢复属于支持证据

数据库故障、断连、重启、事务恢复、资源清理及错误分类 SHALL 在声明生产支持的边界内执行，并保留终态与清理结果。

#### Scenario: 崩溃恢复后存在部分参与者完成
- **WHEN** 一部分参与者已提交而其他参与者状态未知
- **THEN** 矩阵记录真实终态和恢复行为，不要求通过再次提交制造表面一致

### Requirement: 支持数量和平台范围保持真实

公开支持数量 SHALL 只统计 verified/certified profile，并明确数据库版本、驱动 feature 和 Linux/macOS/Windows 范围；一个平台通过不得外推全部平台。

#### Scenario: 仅在 SQLite 或单平台验证
- **WHEN** 某 profile 只有 SQLite 路由或单一操作系统结果
- **THEN** 只能声明该具体边界，不得据此声称全部数据库或全平台支持
