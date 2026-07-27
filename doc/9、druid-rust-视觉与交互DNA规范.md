# druid-rust 视觉与交互 DNA 规范

> **文档说明**：定义 `druid-admin` HTTP 端点的响应形状、错误格式与
> 视觉 DNA（仅 JSON，不内置 Web UI）。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | 视觉与交互 DNA 规范 |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

### 1.1 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [7、领域模型](7、druid-rust-领域模型设计.md) | 聚合名称 |
| [架构文档 §19](../druid-rust-Architecture.zh_CN.md) | 运维端点 |

---

## 2. 范围

druid-rust **不提供** Web UI（参考 [1、命名与品牌](1、druid-rust-命名与品牌说明.md)
§3）。`druid-admin` 仅暴露：

- JSON API：`/druid/api/*`
- Prometheus 文本：`/metrics`

视觉 DNA 适用于任何**消费方**前端（自建 Grafana 仪表盘、内部管理台）。
不在仓库内捆绑静态资源。

## 3. JSON 响应合同

### 3.1 通用信封

```json
{
  "id": "uuid-v4",
  "kind": "DataSourceInfo | SqlStat | WallViolation | ConnectionInfo",
  "fetchedAt": "2026-07-27T12:00:00Z",
  "payload": { /* 见各端点 */ }
}
```

| 字段 | 类型 | 说明 |
| :--- | :--- | :--- |
| `id` | `string` | 响应唯一 ID（`uuid-v4`） |
| `kind` | `string` | 响应类型 |
| `fetchedAt` | `string` | RFC 3339 UTC 时间 |
| `payload` | `object` | 实际数据 |

### 3.2 错误信封

```json
{
  "id": "uuid-v4",
  "kind": "Error",
  "fetchedAt": "2026-07-27T12:00:00Z",
  "error": {
    "code": "AcquireTimeout | PoolInit | SqlParse | WallViolation | DriverError",
    "message": "human-readable",
    "source": "druid-pool | druid-sql | druid-stats | druid-dynamic"
  }
}
```

## 4. 端点清单

| 端点 | 方法 | 响应 `kind` | 说明 |
| :--- | :---: | :--- | :--- |
| `/druid/api/datasources` | GET | `DataSourceInfo[]` | 列出所有数据源与状态 |
| `/druid/api/datasources/:name` | GET | `DataSourceInfo` | 单个数据源详情 |
| `/druid/api/sql/top?limit=N` | GET | `SqlStat[]` | Top N SQL（按 `total_time_ns`） |
| `/druid/api/sql/slow?since=T` | GET | `SqlStat[]` | 慢 SQL（`>= slow_sql_ms`） |
| `/druid/api/wall` | GET | `WallViolation[]` | Wall 拒绝记录 |
| `/druid/api/active` | GET | `ConnectionInfo[]` | 当前活跃连接（无 PII） |
| `/metrics` | GET | `text/plain` | Prometheus 文本 |

## 5. 数据形状

### 5.1 `DataSourceInfo`

```json
{
  "name": "main",
  "driver": "postgres",
  "state": {
    "max_open": 20,
    "in_use": 3,
    "idle": 17,
    "waits": 0,
    "version": 7
  },
  "lastSwitchedAt": "2026-07-27T11:00:00Z"
}
```

### 5.2 `SqlStat`

```json
{
  "sql": "SELECT * FROM users WHERE id = ?",
  "fingerprint": "0xDEADBEEFCAFEBABE",
  "executeCount": 1247,
  "totalTimeMs": 134.5,
  "maxTimeMs": 23.7,
  "errorCount": 3,
  "histogram": {
    "p50": 0.8,
    "p95": 4.2,
    "p99": 12.1
  }
}
```

### 5.3 `WallViolation`

```json
{
  "sql": "DROP TABLE users",
  "fingerprint": "0x...",
  "rules": ["deny_drop_table"],
  "dataSource": "main",
  "occurredAt": "2026-07-27T11:30:00Z"
}
```

### 5.4 `ConnectionInfo`

```json
{
  "id": "0x...",
  "dataSource": "main",
  "state": "Active",
  "heldForMs": 42,
  "useCount": 17,
  "lastSqlFingerprint": "0x..."
}
```

## 6. 视觉 DNA（参考 Druid Java 控制台）

如果团队选择自建 Grafana 仪表盘，建议遵循以下基线（**仅供建议**）：

| 维度 | 值 |
| :--- | :--- |
| 主色 | `#1677FF`（与 Druid Java 控制台一致） |
| 强调色 | `#FF4D4F`（告警 / Wall 拒绝） |
| 中性色 | `#F0F0F0` / `#1F1F1F` |
| 字体 | `Inter`, `system-ui`, `sans-serif` |
| 等宽字体 | `JetBrains Mono`, `Menlo`, `monospace` |
| 数字格式 | 千分位逗号；百分比保留 2 位小数 |
| 时间格式 | RFC 3339 UTC；UI 层可本地化 |

## 7. 标签与命名

| 标签 | 形态 |
| :--- | :--- |
| `data_source` | kebab-case，例：`orders-ro` |
| `sql_fingerprint` | 16 进制 64 位无符号 |
| `driver` | `postgres` / `mysql` / `mssql` / `sqlite` / `duckdb` / `turso` |
| `wall_rule` | snake_case，例：`deny_drop_table` |

## 8. 一致性自检清单

- [ ] 所有 JSON 响应符合 §3 信封。
- [ ] 错误响应符合 §3.2 形态。
- [ ] Prometheus 标签符合 §7。
- [ ] 不返回 SQL 参数字面量。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审