# 8.5：中文主架构图职责边界审查

## 检查对象

主文档：[docs/druid-rust-Architecture.zh_CN.md](../../../../docs/druid-rust-Architecture.zh_CN.md)

第 3 节（第 82–102 行）保留 workspace 总览图，继续展示完整模块关系。第 4.1 节（第 161–184 行）现有一张聚焦的 RDBC 标准合同图，避免从 workspace 总览中推断 JDBC 能力。

聚焦图保留五类用户确认的节点和连线：`应用/ORM → druid（连接池、过滤器、池化代理）→ druid-rdbc`、`druid-wrapper → druid-rdbc`、`druid-wrapper → SQLx / Toasty / RBDC / JDBC Agent 等`。合同组包含 `Connection → Statement → ResultSet`、`PreparedStatement → CallableStatement`、Metadata/SQL 类型/异常、LOB/Array/SQLXML/SQLData，以及 DataSource/Driver/RowSet。

## 职责与图例

- 图前说明跨模块箭头表示调用、实现或依赖；图后说明实线表示访问/接入统一合同。标准分组内两条箭头分别表示操作对象链和接口特化。
- 正文明确 `druid` 持有池化 owner、逻辑租约和回收策略；`druid-wrapper` 执行数据库 I/O 并委托具体后端；`druid-rdbc` 只定义统一标准合同，不拥有池化 owner。
- 图前明确结构图不表示适配器实现全部可选能力；各后端能力由单独的证据矩阵确认。第 3 节仍负责详细 Cargo 依赖方向，避免把合同关系误作 Cargo 直接依赖图。

## 渲染及结构检查

将提交 `4102e09090e58e5fe95cb6b48f89984f7c55624d` 推送到 GitHub 后，通过仓库 Markdown Preview 打开该文件。GitHub Mermaid renderer 为第 4.1 图生成 `viewscreen.githubusercontent.com/markdown/mermaid` 渲染视图；无解析错误，辅助树中可见五个主节点、四组标准分组及其标签。采用该仓库实际 Markdown 渲染器核对，而非仅凭代码围栏推断可渲染。

源码检查确认节点/边、两条标准对象关系、三类合同分组、边界文字和 Mermaid 围栏均在对应位置；`git diff --check` 通过。未安装 Mermaid CLI。

文档 SHA-256：`16B314161FD1864A769E1C0E7743EC4DB905415DCB34A7AF04E6214736CFAC59`

本项只验收架构表达和职责边界。图及其渲染不证明某个后端实现了其中的 JDBC 能力。
