# Admin 短期采样历史核验

对应 OpenSpec：`openspec/changes/admin-bounded-monitor-history`。上一轮控制台实施与 197 项 Admin 基线测试见 `admin-live-console-20261102.md`。本轮只增补真实推送链路中已经存在的数值。

采集来源为 `DruidTelemetrySnapshot` → `SnapshotEntry` → Admin `MetricsRepository`：池活跃、空闲、上限、等待；SQL 累计执行、耗时、取回行、影响行；Wall 检查、拒绝、违规。成功提交时在仓库写锁内追加数值采样。拒绝的批次不追加。计数回退或新的 stream epoch 不产生负变化量。服务端接收时刻用于 TTL，客户端采样时刻只展示。样本总数固定 4096；查询最多 500。历史不保存 SQL、参数、数据库 URL 或驱动异常信息。

`GET /druid/api/v1/history` 使用现有管理认证与脱敏中间件，支持 `serviceId`、`datasourceId`、`limit`。`/health` 同时暴露 `sampleHistory` 容量、已存条数、TTL 和 `durable: false`。采集健康页增加紧凑历史表，不把它作为全部来源的长期趋势；轮询来源目前没有入库历史。

Admin 进程 RSS 是本次额外补齐的真实自监控值：健康 API 按请求从进程采样器读取 `processRssBytes`，只在成功读取时给数值，失败时为 null 并加入 `unavailableFields`。repository 序列化投影字节继续单独显示，不冒充进程内存。此采样不在 SQL 执行路径中。

## 验证与边界

本轮命令与日志保存在 `D:\codex-build\druid-console-live-20261101`。首次运行新增历史测试因缺少 `sample_history` 失败，见 `history-red.log`；RSS 合同测试先因接口仍为空失败，见 `history-rss-red.log`。最终新增 6 项合同测试通过，覆盖提交/拒绝、复合身份、计数回退、容量、TTL、清空、HTTP 参数、管理认证和真实进程内存。前端 Node 行为测试 5 项通过，Vite 生产构建通过，见 `history-vite-final.log`。

`cargo llvm-cov --offline --locked -p druid-admin --tests --lcov` 最终全套运行通过：203 项测试，0 失败；Admin `src/` 行覆盖 4677/6014 = **77.77%**。LCOV 在 `history-rss-coverage-retry.lcov`，完整日志在 `history-rss-coverage-retry.log`。较早的覆盖率运行曾因一项 gRPC 重连时限测试波动失败；它单独复跑通过，后续全套运行也通过。RSS 变更后，一条旧测试仍断言“RSS 必为空”，已按实际可用性合同更新并重跑。上述中途失败均保留日志。构建产物均在 D 盘，未提交/推送，保留并行修改。

嵌入正式 Vue 构建的最新版 Admin 二进制已在 `127.0.0.1:8772` 运行。`/druid/api/v1/history` 返回真实空采样和 `storedSamples: 0`、`maxSamples: 4096`、`ttlMillis: 900000`、`durable: false`；本地没有连接用户业务实例，因此不填充演示数据。`/health` 和实际页面均观测到正数 RSS（样例请求为 12,177,408 字节；该值随进程变化），且不再将 RSS 置于 `unavailableFields`。浏览器检查采集健康页和三种尺寸：1280×1024、768×1024、390×884，`documentElement.scrollWidth` 均等于 `clientWidth`，无页面级横向溢出；浏览器错误日志为空。D 盘的 `history-responsive.json`、`screenshots/history-rss-desktop.png` 和 `screenshots/history-rss-mobile.png` 记录验收状态。历史接口独立失败时不遮蔽原有采集健康数据。

新增合同测试属于 `V1_RUST_LOCAL`；本地正式二进制与 HTTP 页面检查属于指定边界的 `V5_HOST`。这些证据不代表 Java 全量测试迁移或逐案例差分完成。新增 Rust 文件与所改文件用 `rustfmt --edition 2021 --check` 通过；`cargo fmt --all -- --check` 在 Windows 上因项目路径过长（OS 206）未能完成，已记录为工具门禁缺口。`git diff --check` 通过。`sql_parse.rs` 仍有原有未使用变量警告，因此不声称零警告检查门禁通过。

仍缺少跨重启持久化、可配置磁盘容量、长期降采样与分位数、客户端队列/ACK、SQL 错误及直方图的推送协议字段。SQL 累计值只汇总本次快照中实际包含的 SQL 条目，不代表未上报条目的全量执行数。上述指标继续标注未上报；本变更不能替代 Prometheus 或外部时序存储。
