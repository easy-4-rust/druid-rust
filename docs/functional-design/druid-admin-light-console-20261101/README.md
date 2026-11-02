# Druid Admin 浅色专业监控台 · 设计评审包

## 生产落地更新（2026-11-02）

用户已批准修订 04 落地到实际 druid-admin。本次已修改生产 Vue / Rust，继续使用 Vue 3、Element Plus 和 `/druid/api/v1`；产品没有原型数据或演示控件。正式增量事实源为 [admin-live-console](../../../openspec/changes/admin-live-console/proposal.md)，实际验证和剩余缺口见 [生产落地记录](../../verification/admin-live-console-20261102.md)。

后续已加入 [有界短期采样历史](../../../openspec/changes/admin-bounded-monitor-history/proposal.md)：只记录真实推送数值，最近 4096 条，15 分钟 TTL，不跨进程重启；Admin 进程 RSS 也已接入真实采样。验证见 [历史核验](../../verification/admin-bounded-history-20261102.md)。本设计原型中的“历史趋势待接通”是 11 月 1 日状态，不能当作当前生产能力表。

以下为 11 月 1 日设计评审阶段的历史记录，其中“仅设计、未修改生产实现”只描述当时状态。

日期：2026-11-01。基线：Rust `ed161654c203f60e87ea11b4524ca57496a10e73`，Java `fa8dc9912637a2f729eef9f55356621fec18d40e`。

用户已选择「浅色专业监控台，强调可读性和故障定位」，并批准先交付「设计文档＋可点击原型」，保留 Vue 3 / Element Plus 与管理 API。**这批准了设计工作与视觉方向；下列新增 API、采集字段、权限和历史存储仍是待评审方案，不代表生产功能已交付。**

## 阅读入口

- [剩余迁移工作与分类核对](remaining-work.md)：先核对完成了什么，再规划真实缺口。
- [页面、指标和数据链路设计](design.md)：信息架构、字段语义、交互、状态、兼容性和验收。
- [可点击原型](prototype.html)：浏览器打开即可，无外部依赖。固定演示数据，不连接数据库，也不执行管理命令。
- [验证记录](verification.md)：本轮后端基线、原型检查与覆盖率边界。

原型覆盖主页、数据源、SQL 统计、慢 SQL、防火墙、API 索引、采集健康、驱动能力，以及 SQL / 数据源详情抽屉；支持服务 / 数据源筛选、SQL 搜索、排序和分页、状态演示、暂停刷新和键盘关闭抽屉。实例与能力的演示标记始终可见。未承诺的事务协调、通知投递和统计重置不提供假成功按钮。

原型使用自包含 HTML 方便直接评审，不引入生产第二套前端；正式落地仍使用现有 Vue 3 / Element Plus。

当前原型为 **中间件控制台修订 04**：以用户提供的原稿为视觉基准，恢复深色顶部导航、紧凑表格、四组行内直方图、排序与分页；撤销前两版通用仪表盘方向。界面继续保留实例身份与数据缺失提示。主页是默认入口，SQL 仍为主要排障工作页。主页提供紧凑运行摘要、问题定位、实例、连接池和错误 SQL；其他页面补充生命周期、配置、连接与 SQL 详情、Wall 表/函数/名单及采集链路指标。生产 Vue/Rust 实现未改动。

## 规格关系

这是单模块设计评审材料，**不是第二套正式规格**。正式事实源仍为：

- [admin-web-console](../../../openspec/specs/admin-web-console/spec.md)
- [admin-runtime](../../../openspec/specs/admin-runtime/spec.md)
- [metrics-runtime](../../../openspec/specs/metrics-runtime/spec.md)
- [ADR-CRATE-003](../../migration/adr/ADR-CRATE-003.md)

历史执行计划继续留在 `docs/superpowers/`；本设计不改写其历史通过数，也不把旧的未勾选项直接当作缺失实现。后续实施前，在既有 OpenSpec 下建立一个增量 change，引用本设计中已获批准的内容，不另建冲突需求。

本轮不修改产品 Rust / Vue 实现，不更换分支，不安装依赖，不提交或推送；构建、测试、临时文件均在 D:。保留工作区并行修改。全仓逐方法语义迁移没有在本轮重新完成，详见剩余工作台账。
