# Druid Admin Web

## 真实监控控制台（2026-11-02）

默认主页 `/druid/console/#/overview`，沿用深色顶部导航、浅色紧凑表格。页面全部消费 v1 API；不连接原型、不在请求失败时退回示例数据。支持数据源运行/生命周期/配置、SQL 排序/搜索/分页/详情、慢 SQL、Wall、采集健康、驱动能力和 API 索引。

缺失字段通过 `unavailableFields` 转为 `null`，显示“未上报”；真实零保留为零。详情携带来源上下文，冲突返回 409。慢 SQL 指累计最大执行时间达到阈值的 SQL 条目，不代表慢请求次数或比例。连接与堆栈在数据源详情按需读取。

### 构建和验证（产物放 D 盘）

```powershell
$env:DRUID_CONSOLE_DIST = 'D:\codex-build\druid-console-live-20261101\dist'
npm test
npm run build
# 回到仓库根目录，保持同一个 DRUID_CONSOLE_DIST
$env:CARGO_TARGET_DIR = 'D:\codex-build\druid-audit-20261101'
cargo build --locked -p druid-admin --bin druid-admin
```

`build.rs` 内嵌指定目录；未设置时保留 `web/dist` 默认行为。开发代理可用 `DRUID_ADMIN_URL` 指向实际 Admin（默认 `http://127.0.0.1:8080`）。认证和脱敏继续由服务端执行，浏览器不存储管理口令。

API 可用性和测试边界见仓库 `docs/verification/admin-live-console-20261102.md`。

采集健康页另读取 `/druid/api/v1/history` 的真实推送样本。它保留最近 4096 条数值快照，显示连接池、SQL 累计/变化量与 Wall 计数；默认取最近 100 条，最多 500 条。该历史与最新投影一样是进程内短期状态，重启后清空，详见 `openspec/changes/admin-bounded-monitor-history`。

同页的“进程 RSS”由 Admin 进程内存采样器直接读取；采样不可用时显示“未上报”。仓库序列化字节与 RSS 是不同指标。
