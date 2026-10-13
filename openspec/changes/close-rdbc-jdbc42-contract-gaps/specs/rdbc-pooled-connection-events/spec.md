# 池化连接事件

## Purpose

为仅依赖标准池化连接接口的外部连接池提供完整监听器接入，使逻辑连接归还、致命错误和预编译语句事件具有稳定身份及通知规则，而无需依赖 Druid 内部类型。

## ADDED Requirements

### Requirement: PE-01 标准接口提供四个监听器操作

标准 PooledConnection SHALL 提供连接监听器和语句监听器的添加、移除入口，使用监听器对象身份进行匹配；重复注册与移除次数 SHALL 对齐冻结的 Druid 1.2.28 事件行为并记录在方法级映射中。

#### Scenario: 外部池仅依赖标准接口
- **WHEN** 外部实现通过标准池化连接对象添加并移除两类监听器
- **THEN** 无需下转型至 Druid holder，且编译与运行均可完成

#### Scenario: 移除未注册监听器
- **WHEN** 调用方移除一个未注册的监听器
- **THEN** 不触发事件、不改变其他监听器注册状态，返回与标准映射一致的结果

### Requirement: PE-02 逻辑关闭和连接错误分别通知

系统 SHALL 在逻辑连接有效关闭时通知连接关闭事件，在判定连接不可继续使用的数据库错误时通知连接错误事件；事件 SHALL 携带实际 pooled owner 身份和完整 SQL 异常，重复关闭不能重复产生同一次归还通知。

#### Scenario: 逻辑关闭与物理关闭分离
- **WHEN** 应用关闭从 pooled owner 获取的逻辑连接
- **THEN** 关闭事件关联原 owner，物理连接按池策略保留；关闭 pooled owner 时才释放物理资源

#### Scenario: 致命错误携带原因
- **WHEN** 连接执行发生由错误分类器判定的致命 SQL 错误
- **THEN** 连接错误监听器收到完整异常，失败连接不被重新借出

### Requirement: PE-03 预编译语句事件保留标准身份

系统 SHALL 为受池管理的 PreparedStatement 及其 CallableStatement 子类型提供关闭/失效事件；事件源 SHALL 是真实 pooled owner，并包含原语句身份及可用的 SQL 异常，不能用新建空对象替代。

#### Scenario: 关闭预编译语句
- **WHEN** 受池管理的预编译语句有效关闭
- **THEN** 对当前注册项通知一次对应事件，关闭已关闭语句不重复通知同一次关闭

#### Scenario: 驱动报告语句不可用
- **WHEN** 驱动确认某预编译语句失效
- **THEN** 失效事件携带原语句和异常，已移除的监听器不会在随后新事件中被调用

#### Scenario: Prepared 执行失败的事件顺序
- **WHEN** PreparedStatement 或 CallableStatement 执行返回 SQL 异常
- **THEN** 系统先完成错误分类及 fatal owner 处置（若适用），再发送语句错误事件，最后将原 SQL 异常返回调用方

#### Scenario: 成功关闭后只通知一次
- **WHEN** PreparedStatement 或 CallableStatement 底层关闭成功
- **THEN** 系统先发送一次语句关闭事件，再注销语句资源；重复逻辑关闭不重复通知，底层关闭失败不发送成功事件

### Requirement: PE-04 通知支持重入且不阻断资源处置

系统 SHALL 在不持有注册表或池资源锁的情况下通知监听器；一次通知使用固定快照，回调中的注册变更 SHALL 从下一次通知生效；回调失败不得阻止 owner 完成必要清理。

#### Scenario: 回调移除自身
- **WHEN** 监听器在事件回调中移除自身或注册另一个监听器
- **THEN** 无死锁，当前快照不被重排，新注册项不追溯收到当前事件

#### Scenario: 回调发生 panic
- **WHEN** 可展开的监听器回调发生 panic
- **THEN** 必要资源清理仍完成，错误进入受控诊断；panic=abort 的进程终止不被声明为可恢复场景

#### Scenario: 逻辑连接归还通知顺序
- **WHEN** 应用关闭逻辑连接
- **THEN** 系统先发送一次 connection-closed 通知，再开始 owner reset/recycle 或 discard；该通知本身不表示物理连接已可再次借出
