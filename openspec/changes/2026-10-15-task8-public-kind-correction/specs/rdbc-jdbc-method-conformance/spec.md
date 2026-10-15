## ADDED Requirements

### Requirement: MC-07 Rust 公开形态与冻结 Java 身份分别校验

JDBC 对象清单 SHALL 保留冻结 Java 8 对象身份、签名和来源摘要，同时将 Rust 形态和 canonical path 绑定当前公开 API 的真实声明。已审阅的形态或路径迁移 SHALL 以逐项固定映射纳入 oracle 规范化，不能批量放宽源码摘要或把静态声明提升为适配器、逻辑代理及能力运行证据。带 `#[non_exhaustive]` 的公开枚举 SHALL 被识别为枚举；关联常量 SHALL NOT 冒充枚举成员。

#### Scenario: 六个类型形态和授权分类路径迁移
- **WHEN** 当前 RDBC public API 对五个异常载荷及 `DriverRegistry` 暴露 struct，对授权分类暴露真实枚举成员
- **THEN** 清单使用相应 DTO 形态和真实成员路径；冻结 Java 来源校验仍通过，其他字段和 PLANNED 证据状态不变

#### Scenario: 带属性枚举与关联常量
- **WHEN** public API 含 `#[non_exhaustive] pub enum` 及其真实成员，或只含同名关联常量
- **THEN** 前者通过形态校验，后者不能满足 `ENUM_KIND` 成员要求
