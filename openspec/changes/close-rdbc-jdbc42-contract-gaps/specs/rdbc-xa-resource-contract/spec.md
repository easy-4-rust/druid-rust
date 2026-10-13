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
