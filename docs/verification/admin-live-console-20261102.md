# druid-admin 真实控制台落地与验证

日期：2026-11-02。变更：`openspec/changes/admin-live-console`。正式规格：`openspec/specs/admin-web-console/spec.md`，继续沿用 `admin-runtime` 安全和部署合同。基于 Rust `ed161654c203f60e87ea11b4524ca57496a10e73` 的工作区增量；保留此前设计文档和并行修改，没有提交或推送。

## 已实现

- 生产 Vue 3 / Element Plus 主页、深色顶部导航和紧凑表格，八个主页面；原型不打包进入产品。
- 主页直接读取真实连接池和 SQL 统计：运行摘要、等待/错误定位、服务实例与连接池、错误 SQL 入口。
- 保留原有数据源和 SQL 完整详情、SQL 复制、解析结果、四组直方图、Wall 表/函数/名单、排序和分页。新增数据源生命周期/配置页签、按来源精确跳转、连接列表/PS 缓存/借出堆栈按需读取。
- 采集健康读取 repository 接收/拒绝/容量/TTL/序列/epoch/最近接收经过时间；驱动能力基于实际来源的字段可用性显示。
- 刷新保留组件、页签、阈值、搜索、排序与分页；旧请求结果不能覆盖新请求，卸载后不提交结果。
- 缺失指标与真实零严格区分；未接入原文、最大耗时、直方图不用于相应搜索或慢 SQL 判断。慢 SQL 删除错误的“慢执行占比”口径，采用服务端先筛选后分页。

## API 与兼容性

新增 `GET /druid/api/v1/health`、`/datasources/{identity}/connections`、`/datasources/{identity}/stack`。列表/详情添加来源、`unavailableFields`、`partial`、`failures` 和 `observedAt`，保留旧 camelCase 数值字段供旧客户端使用；新 UI 根据可用性屏蔽 wire 默认值。现有 Java `.json` 路由及其远端优先语义不变。

v1 同时读取 poll 和 repository，并保留来源身份；不因一个来源成功而吞掉其他来源失败。全部来源失败为 502；部分失败保留可用数据并显示警告；没有接入来源是成功的空状态。数值身份不唯一返回 409；详情使用 `serviceId`，SQL 还使用 `datasourceId`。`sql` 支持 `search`、`minMaxTimespan`、`serviceId`、`serviceName`、`datasourceId`；先过滤，再稳定排序、分页，单页最多 500。未知指标排在已知指标之后。

单个 poll 节点一次最多读取 10,000 个 SQL 条目；达到预算明确返回部分结果警告，不能把其 total 当完整节点总数。当前 Wall 页面是全部来源汇总，显示各字段覆盖来源数，不跟随数据源筛选。现有鉴权、非本机 TLS 约束、服务端敏感信息策略沿用，未添加管理写操作。

## 测试证据

构建、日志、覆盖率和截图统一存于 `D:\codex-build\druid-console-live-20261101`。

1. TDD 初次执行：前端缺失值/身份参数共 3 个断言失败；新 Rust 合同初版 4 失败、1 通过，暴露缺失字段元数据、409 身份消歧、健康接口和筛选分页缺口。`red.log` 保留 Rust 失败输出。
2. 前端 Node 行为测试：5 通过。覆盖缺失值与零、空/零直方图区分、详情参数编码、迟到请求与卸载失效。`node --test crates/druid-admin/web/tests/*.test.js`。
3. 新 Rust HTTP 合同：7 通过。覆盖 push 可用性、同号身份、采集计数与时间、筛选/页长/未知慢耗时、已知 push 不支持连接、poll 零/缺失字段/解析/堆栈/极大页码、完全及部分采集失败。只在测试中使用受控节点 fixture；生产没有 fixture 数据。
4. 全部 Admin 测试经 `cargo llvm-cov --offline --locked -p druid-admin --tests --lcov --output-path D:\codex-build\druid-console-live-20261101\coverage.lcov`：**197 通过，0 失败，0 忽略**。复用原有鉴权、TLS、Java 兼容 DTO/接口、repository 限额、协议和脱敏测试。
5. 覆盖率从 LCOV 按 `crates/druid-admin/src/` 精确汇总：**行 4522/5858 = 77.19%；函数 527/706 = 74.65%**。本次四个读取文件（v1_api、v1_snapshot、v1_health、console_read）合计 **682/738 行 = 92.41%**。不是全 workspace 覆盖率；分支覆盖未启用。明细 `coverage-summary.json`。
6. Vite 生产构建成功；真实 druid-admin 二进制嵌入 D 盘产物成功。现有 Element Plus 主包仍有大于 500 kB 的构建提示，原有 `sql_parse.rs` 未使用变量警告未在本轮掩盖。
7. 实际生产二进制 `http://127.0.0.1:8770/druid/console/#/overview` 完成页面验收。八页面导航可达，数据源生命周期页签、SQL 搜索、慢 SQL 阈值在刷新后保留；健康页显示真实 repository 零值/容量预算，RSS/ACK/队列明确未上报。浏览器未捕获运行错误。
8. Desktop 1280×1024、Tablet 768×1024、Mobile 390×884 检查；平板/手机八页面均没有页面级横向溢出，宽表与顶栏局部滚动。截图 `screenshots/home-desktop.png`、`home-768.png`、`home-390.png`；尺寸结果 `responsive.json`。
9. `openspec validate admin-live-console --strict` 和 `git diff --check` 通过。

## 真实缺口与验收边界

- 本地运行实例没有连接用户业务应用。截图反映真实空状态；有数据的 poll/push 合同用 HTTP 集成测试验证。尚未完成用户实际业务集群的端到端接入验收，不能称作已上线。
- 当前 push wire 没有 SQL 原文、最大耗时、错误详情、直方图、完整池配置、PS 缓存和连接诊断。UI 明确缺失；这些数据在支持的 poll 节点上沿用已有能力。驱动版本、RSS、ACK/队列/未刷出数未接入。不为改版伪造，也不为视觉改版修改 SQL 执行热路径。
- 采集健康是当前快照，不提供历史趋势存储、告警通知、2PC 管理或统计重置。repository 状态字节是序列化投影大小，不是进程 RSS；客户端采样时间与服务端接收经过时间分别显示。

后续增量已补进程内短期推送采样历史、版本化读取接口和真实 Admin 进程 RSS；上述“无历史”和“RSS 未接入”是本报告基线时的状态。当前状态、容量和覆盖率见 [Admin 短期采样历史核验](admin-bounded-history-20261102.md)。跨重启持久化与长期趋势仍未实现。
- 现有 OpenAPI 索引能列出所有路由，但仍是手写摘要，尚不满足全参数/响应 schema 与路由同源生成要求。这是已有真实缺口，不算本次已完成。
- 全仓 Java 逐方法差分、全部原始测试迁移、所有数据库适配能力仍以原迁移核对台账为准。197 项 Admin 回归和本地覆盖率不等价于完整迁移通过。

## 当前规格状态

本增量的生产读取、页面和本地验证完成，并同步正式需求；保留 change 供审阅，不自动归档。后续按真实缺口补 wire / 自监控暴露与目标业务集群验收。
