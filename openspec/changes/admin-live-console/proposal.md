# 真实中间件控制台

按已批准的 revision 04 视觉设计落地现有 Vue 3 / Element Plus 应用。保留现有管理 API、配置、SQL 复制/解析/直方图、防火墙名单与兼容路由；增加真实主页、采集健康与能力视图。

原型固定数据、演示开关、设计解释不进入产品。未上报/不支持、失败、空数据必须不同。默认零不能证明被采集；累计最大耗时只用于“曾慢 SQL”，不等于窗口慢请求比例。

范围：druid-admin 控制台与其版本化读取合同；不修改数据库执行路径或 protobuf，不新增告警投递、历史库、2PC 操作。

复用 docs/functional-design/druid-admin-light-console-20261101/design.md；正式事实源仍为 openspec/specs/admin-web-console 和 admin-runtime。本次授权为生产实施，不再停留原型。
