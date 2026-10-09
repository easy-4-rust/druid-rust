from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


def replace_once(text, old, new):
    assert text.count(old) == 1, repr(old)
    return text.replace(old, new, 1)


def section(text, start, end, replacement):
    assert text.count(start) == text.count(end) == 1
    left = text.index(start)
    right = text.index(end, left)
    return text[:left] + replacement.rstrip() + '\n\n' + text[right:]


graph = '''```mermaid
flowchart TD
    WRAPPER["druid-wrapper<br/>Toasty / SQLx / RBDC / external pools"] --> DRUID["druid<br/>Pool / Filter / SQL / Wall / Stat / Dynamic"]
    WRAPPER --> RDBC["druid-rdbc<br/>Connection / Statement / ResultSet / resources"]
    DRUID --> RDBC
    METRICS["druid-metrics<br/>sampling / push / gRPC protocol"] --> DRUID
    ADMIN["druid-admin<br/>HTTP / gRPC / repository / authentication"] --> METRICS
    ADMIN --> DRUID
    ADMIN -. "optional driver bootstrap" .-> WRAPPER
```'''

en4 = '''<a id="4-three-module-architecture-and-five-crate-target"></a>

## 4. RDBC-First Five-Product-Crate Architecture

### 4.1 Current module responsibilities

ADR-CRATE-002 supersedes the former ADR-CRATE-001 `druid-core + facade` target.
The five product crates already exist; their presence does not establish release readiness.

| Product crate | Responsibility | Main production dependencies |
| :--- | :--- | :--- |
| `druid-rdbc` | Independent standard traits, errors, metadata, resources and Wrapper identity | No Druid product dependency |
| `druid` | Pool, holder, logical proxies, Filter, SQL, Wall, Stat and Dynamic | druid-rdbc |
| `druid-wrapper` | Toasty/SQLx/RBDC/DuckDB/libSQL/HTTP SQL/JDBC Agent and external-pool integration | druid, druid-rdbc |
| `druid-metrics` | Local observation, sampling, push and shared gRPC protocol | druid |
| `druid-admin` | HTTP/gRPC management, repository, authentication, static UI and standalone binary | druid, druid-metrics; optional druid-wrapper |

`druid-bench` is the sixth workspace member and a verification helper, not a sixth product crate.
Toasty integration belongs to `druid-wrapper`; its default features are `driver-catalog` and `sqlite`.

### 4.2 Dependency direction

Arrows point from a dependent crate to its dependency; optional edges are dashed.

''' + graph + '''

### 4.3 Connection ownership and dispatch

```text
Driver / ConnectionFactory
    -> Box<dyn druid_rdbc::connection::Connection>
        -> DruidConnectionHolder
            -> DruidPooledConnection
                -> pooled Statement / PreparedStatement / CallableStatement / ResultSet
```

Wrappers and Druid logical proxies implement the same standard trait tree. Druid adds
Filter dispatch, statistics, prepared cache and lifecycle policy around that contract.
bb8/deadpool implement the `Pool` acquisition contract and retain their original pool
authority through `ExternalConnectionLease`; they must not be nested as factories inside `DruidPool`.

### 4.4 Verification boundary

Current source and configuration were checked on 2026-10-08. P04 Task6 remains
FIX_REQUIRED; Task7, Task8 and P05–P07 remain open. Source checks and historical
package passes do not establish current full-workspace or release acceptance.
See the [architecture/progress audit](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md)
and [authoritative verification status](docs/verification/rdbc-first/status.md).
'''

zh4 = '''<a id="4-三模块架构与五目标"></a>

## 4. RDBC-First 五产品 Crate 架构

### 4.1 当前模块职责

ADR-CRATE-002 已取代 ADR-CRATE-001 的 `druid-core + facade` 目标。
五产品 crate 已存在，模块存在不代表达到发布验收标准。

| 产品 crate | 职责 | 主要生产依赖 |
| :--- | :--- | :--- |
| `druid-rdbc` | 独立标准接口、错误、metadata、资源与 Wrapper 身份 | 不依赖 Druid 产品实现 |
| `druid` | 池化、holder、logical proxy、Filter、SQL、Wall、Stat、Dynamic | druid-rdbc |
| `druid-wrapper` | Toasty/SQLx/RBDC/DuckDB/libSQL/HTTP SQL/JDBC Agent 与外部池适配 | druid、druid-rdbc |
| `druid-metrics` | 本地观测、采样、推送与公共 gRPC 协议 | druid |
| `druid-admin` | HTTP/gRPC 管理、repository、认证、静态 UI 和独立 binary | druid、druid-metrics；可选 druid-wrapper |

`druid-bench` 是第六个 workspace 成员，负责验证，不属于第六个产品 crate。
Toasty 集成归属 `druid-wrapper`；其默认 features 为 `driver-catalog` 与 `sqlite`。

### 4.2 依赖方向

箭头从使用方指向依赖方；虚线为可选依赖。

''' + graph + '''

### 4.3 连接所有权与调用链

```text
Driver / ConnectionFactory
    -> Box<dyn druid_rdbc::connection::Connection>
        -> DruidConnectionHolder
            -> DruidPooledConnection
                -> pooled Statement / PreparedStatement / CallableStatement / ResultSet
```

Wrapper 与 Druid logical proxy 实现同一标准接口树。Druid 围绕该合同加入 Filter、
统计、prepared cache 和生命周期策略。bb8/deadpool 实现 `Pool` 获取合同，通过
`ExternalConnectionLease` 保留原连接池权威，不能作为 factory 再嵌套进 `DruidPool`。

### 4.4 验收边界

当前源码与配置核验日期为 2026-10-08。P04 Task6 仍为 FIX_REQUIRED，Task7、Task8
及 P05–P07 仍开放。源码检查与历史 package 通过不能证明当前全仓或发布验收通过。
详见[架构进度核验](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md)
与[正式验证状态](docs/verification/rdbc-first/status.md)。
'''

en7 = '''## 7. Cargo Features

Features are owned by their product crate. `druid` has an empty default feature set;
it depends on `druid-rdbc` and its runtime libraries, not on a `druid-core` facade.

| Crate / feature | Default | Current behavior |
| :--- | :---: | :--- |
| `druid` / `config-http` | No | Optional HTTP configuration dependency |
| `druid` / `sqlite`, `postgresql`, `mysql`, `turso` | No | Empty feature entries; they do not activate a database driver |
| `druid-wrapper` / `driver-catalog`, `sqlite` | Yes | Driver catalog and Toasty SQLite integration |
| `druid-wrapper` / `postgresql`, `mysql`, `turso` | No | Toasty provider feature selection |
| `druid-wrapper` / `duckdb-native`, `http-sql`, `libsql-native` | No | Optional native/HTTP driver dependencies |
| `druid-wrapper` / `jdbc-agent`, `managed-driver-install` | No | JDBC Agent bridge and managed driver tooling |
| `druid-metrics` / `client`, `server` | No | gRPC client/server dependencies |

```bash
cargo check -p druid --no-default-features
cargo check -p druid-wrapper --features sqlite
cargo check -p druid-metrics --features client
cargo check -p druid-wrapper --all-features
```

These are usage examples, not proof that the current feature matrix passes. SQLx/RBDC/
bb8/deadpool remain non-optional wrapper dependencies; complete feature isolation and
feature-combination verification are open P05 requirements. Non-SQL Toasty providers
do not enter the Druid SQL database support count.
'''

zh7 = '''## 7. Cargo Features

Feature 由所属产品 crate 管理。`druid` 的默认 feature 为空，依赖 `druid-rdbc`
与运行时组件，不是依赖 `druid-core` 的 facade。

| Crate / feature | 默认启用 | 当前行为 |
| :--- | :---: | :--- |
| `druid` / `config-http` | 否 | 可选 HTTP 配置依赖 |
| `druid` / `sqlite`、`postgresql`、`mysql`、`turso` | 否 | 空 feature 声明，不会激活数据库驱动 |
| `druid-wrapper` / `driver-catalog`、`sqlite` | 是 | 驱动目录与 Toasty SQLite 集成 |
| `druid-wrapper` / `postgresql`、`mysql`、`turso` | 否 | Toasty provider feature 选择 |
| `druid-wrapper` / `duckdb-native`、`http-sql`、`libsql-native` | 否 | 可选原生/HTTP 驱动依赖 |
| `druid-wrapper` / `jdbc-agent`、`managed-driver-install` | 否 | JDBC Agent 桥接与驱动管理工具 |
| `druid-metrics` / `client`、`server` | 否 | gRPC 客户端/服务端依赖 |

```bash
cargo check -p druid --no-default-features
cargo check -p druid-wrapper --features sqlite
cargo check -p druid-metrics --features client
cargo check -p druid-wrapper --all-features
```

以上是用法示例，不是当前 feature 矩阵通过证据。SQLx/RBDC/bb8/deadpool 仍是
wrapper 的非 optional 依赖；完整 feature 隔离和组合验证属于尚未关闭的 P05。
非 SQL Toasty provider 不计入 Druid SQL 数据库支持数。
'''

updates = []
for filename, architecture, features in [('README.md', en4, en7), ('README.zh-CN.md', zh4, zh7)]:
    path = ROOT / filename
    raw = path.read_bytes()
    newline = '\r\n' if b'\r\n' in raw else '\n'
    text = raw.decode('utf-8').replace('\r\n', '\n')
    start = '## 4. Three-Module Architecture and Five-Crate Target' if filename == 'README.md' else '## 4. 三模块架构与五目标'
    end = '## 5. Design Principles' if filename == 'README.md' else '## 5. 设计原则'
    text = section(text, start, end, architecture)
    end = '## 8. Core API and Usage' if filename == 'README.md' else '## 8. 核心 API 与用法'
    text = section(text, '## 7. Cargo Features', end, features)
    text = text.replace('druid_rdbc::Connection>', 'druid_rdbc::connection::Connection>')
    text = text.replace('DruidPooledConnection → DruidConnectionHolder → PhysicalConnection', 'DruidPooledConnection → DruidConnectionHolder → druid_rdbc::connection::Connection')
    text = text.replace('PhysicalConnectionFactory', 'ConnectionFactory').replace('PhysicalConnectionLease', 'ExternalConnectionLease').replace('PhysicalConnectionCapabilities', 'ConnectionCapabilities')
    text = text.replace('cargo run -p druid-admin --bin druid-driver --', 'cargo run -p druid-wrapper --features managed-driver-install --bin druid-driver --')
    if filename == 'README.md':
        pairs = [
            ('[Three-module architecture]', '[Five-product-crate architecture]'),
            ('> **Last verified:** 2026-08-07.', '> **Architecture/configuration checked:** 2026-10-08. Runtime and release gates remain open; see the [current audit](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md).'),
            ('| Current transitional raw contract | `PhysicalConnection*`, pending merge/delete under ADR-RDBC-006 |', '| Current raw contract | `druid_rdbc::connection::Connection`; final Task6 deletion/API/runtime acceptance remains open |'),
            ('| Internal `PhysicalConnection` SPI | 🚧 Partial | `druid` | metadata/LOB/vendor surface pending | physical contract |', '| Standard `Connection` contract | 🚧 Partial | `druid-rdbc` | lifecycle/resource/capability certification pending | standard contract |'),
            ('| Java-compatible `/druid/*` Admin | 🗓️ Planned | `druid-admin` | only placeholder state/endpoint strings | migration ledger |', '| Java-compatible `/druid/*` Admin | 🚧 Partial | `druid-admin` | real HTTP/gRPC services; current network acceptance remains open | verification status |'),
            ('The current `Physical*` names are transitional migration inputs, not a second approved standard.', 'Retired `Physical*` names belong to migration history. Current adapters and logical proxies use the single standard; complete Task6 acceptance remains open.'),
            ('| `PhysicalConnection` | Minimal internal SPI for direct adapters |', '| `druid_rdbc::connection::Connection` | Standard contract implemented by direct adapters and pooled proxies |'),
            ('The old `Connection`, `ConnectionFactory`, and `PooledConnection` names remain only as\nmigration compatibility re-exports and are not canonical for new code.', 'Use `druid_rdbc::connection::Connection` and `druid_rdbc::driver::ConnectionFactory`\nfor the standard boundary, and `DruidPooledConnection` for the product pooled handle.'),
            ('`druid-admin` remains a placeholder and has no real Axum Router or handlers. Endpoint\nlist tests do not establish a working HTTP service.', '`druid-admin` has real Topcoat/Axum routes and HTTP/gRPC services. Check listener,\nauthentication/TLS configuration and the latest network-test evidence. Endpoint strings\nalone do not prove reachability; current network acceptance remains open.'),
            ('No module is published. Only `druid`, `druid-wrapper`, and `druid-admin` may ultimately\nbe published. Minimum publication conditions:', 'All workspace packages currently set `publish = false`. The five planned product\npackages are `druid-rdbc`, `druid`, `druid-wrapper`, `druid-metrics` and `druid-admin`;\n`druid-bench` is a verification helper. Minimum publication conditions:'),
        ]
    else:
        pairs = [
            ('[三模块架构与五目标]', '[五产品 Crate 架构]'),
            ('> **最后核验**：2026-08-07。', '> **架构与配置核验**：2026-10-08。运行时和发布门禁仍开放，详见[当前核验](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md)。'),
            ('| 当前过渡 raw 合同 | `PhysicalConnection*`，按 ADR-RDBC-006 等待 merge/delete |', '| 当前 raw 合同 | `druid_rdbc::connection::Connection`；Task6 删除/API/运行验收仍开放 |'),
            ('| `PhysicalConnection` 内部 SPI | 🚧 部分 | `druid` | metadata/LOB/vendor 能力待扩 | physical contract |', '| 标准 `Connection` 合同 | 🚧 部分 | `druid-rdbc` | 生命周期/资源/能力认证待完成 | standard contract |'),
            ('| `/druid/*` Admin 兼容面 | 🗓️ 计划 | `druid-admin` | 只有占位 state/endpoint 字符串 | 迁移账本 |', '| `/druid/*` Admin 兼容面 | 🚧 部分 | `druid-admin` | 已有真实 HTTP/gRPC 服务；当前网络验收仍开放 | 验证状态 |'),
            ('当前 `Physical*` 名称仅为迁移输入，不是第二套已批准标准。', '已退役的 `Physical*` 名称属于迁移历史。当前 adapter 与 logical proxy 使用唯一标准；Task6 完整验收仍开放。'),
            ('| `PhysicalConnection` | direct adapter 的内部最小 SPI |', '| `druid_rdbc::connection::Connection` | direct adapter 与 pooled proxy 实现的标准合同 |'),
            ('旧 `Connection`、`ConnectionFactory`、`PooledConnection` 名称只保留迁移期兼容\n重导出，不是新代码的 canonical 名称。', '标准边界使用 `druid_rdbc::connection::Connection` 与\n`druid_rdbc::driver::ConnectionFactory`；产品池化对象使用 `DruidPooledConnection`。'),
            ('`druid-admin` 当前仍是占位实现，没有真实 Axum Router/handler。endpoint list\n测试不代表 HTTP 服务完成。', '`druid-admin` 已有真实 Topcoat/Axum 路由与 HTTP/gRPC 服务。排查监听地址、\n认证/TLS 配置与最新网络测试证据。endpoint 字符串不能证明端口可达，当前网络验收仍开放。'),
            ('当前没有模块发布。最终只能发布 `druid`、`druid-wrapper`、`druid-admin`。\n发布前至少满足：', '当前所有 workspace package 均为 `publish = false`。五个计划产品 package 为\n`druid-rdbc`、`druid`、`druid-wrapper`、`druid-metrics`、`druid-admin`；\n`druid-bench` 负责验证。发布前至少满足：'),
        ]
    for old, new in pairs:
        text = replace_once(text, old, new)
    updates.append((path, text.replace('\n', newline).encode('utf-8')))

# 两种语言全部原文匹配成功后才落盘，避免单边更新。
for path, payload in updates:
    path.write_bytes(payload)
    print('UPDATED', path.name)
