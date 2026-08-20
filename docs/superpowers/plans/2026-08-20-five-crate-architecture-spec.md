# Five-Crate Architecture Specification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将五 Crate 目标写入正式架构事实源，废止当前“三 Crate only”结论，并为后续源码迁移提供唯一、无冲突的职责和依赖定义。

**Architecture:** 目标 workspace 包含 `druid-core`、`druid`、`druid-wrapper`、`druid-metrics`、`druid-admin`。`druid-core` 是无具体驱动和无管理传输的核心；`druid` 是稳定门面；Wrapper 和 Metrics 分别从 Core 向上扩展；Admin 只消费 Metrics 协议。

**Tech Stack:** Markdown、Mermaid、Cargo workspace、Superpowers SDD

**Spec:** `docs/druid-rust-Architecture.zh_CN.md`

## Global Constraints

- 规格事实源只允许 `docs/superpowers/specs/`、`docs/superpowers/plans/` 和总体架构文档。
- 保留全部既有完成证据；目标路径变化不能自动把 PARTIAL 改成 DONE。
- `druid::sql::*` 保持稳定 RDBC API。
- 严禁 Git worktree、自动切分支、reset、commit 或 push。

---

### Task 1: 新增五 Crate ADR

**Files:**
- Modify: `docs/druid-rust-Architecture.zh_CN.md`
- Modify: `docs/superpowers/specs/2026-08-12-connection-abstraction-design.md`
- Test: `docs/superpowers/plans/2026-08-20-five-crate-architecture-spec.md`

**Interfaces:**
- Produces: `ADR-CRATE-001`、`ADR-METRICS-001`、`ADR-TRANSPORT-001`、`ADR-ADMIN-001`
- Supersedes: `ADR-013`、`ADR-CONN-008`

- [ ] **Step 1: 写入架构失败检查**

```bash
rg -n "ADR-CRATE-001|ADR-METRICS-001|ADR-TRANSPORT-001|ADR-ADMIN-001" \
  docs/druid-rust-Architecture.zh_CN.md \
  docs/superpowers/specs/2026-08-12-connection-abstraction-design.md
```

Expected: no matches before the change.

- [ ] **Step 2: 增加 ADR 和依赖图**

必须写明：

```text
druid-core -> druid-wrapper
druid-core -> druid-metrics
druid-core -> druid facade
druid-metrics -> druid-admin
druid-admin -X-> druid-wrapper
```

并明确 `druid` 可选依赖 Metrics/Wrapper 的 Cargo 方向不会形成循环。

- [ ] **Step 3: 标注旧 ADR 已废止**

在旧 ADR 原位置保留历史说明，状态改为 `SUPERSEDED_BY ADR-CRATE-001`，不得直接删除历史。

- [ ] **Step 4: 复查 ADR 可检索**

Run the Step 1 command again. Expected: each new ADR has at least one definition and one reference.

### Task 2: 更新治理、对象与驱动归属

**Files:**
- Modify: `docs/superpowers/specs/2026-08-12-migration-governance.md`
- Modify: `docs/superpowers/specs/2026-08-12-object-and-semantic-mapping.md`
- Modify: `docs/superpowers/specs/2026-08-12-object-naming-audit.md`
- Modify: `docs/superpowers/specs/2026-08-12-driver-integration-matrix.md`

**Interfaces:**
- Produces: 每个现有对象的目标 crate 归属
- Consumes: Task 1 的五 Crate ADR

- [ ] **Step 1: 更新对象归属表**

精确归属：

```text
Core: RDBC/JDBC 类型、Pool、Filter、SQL、Wall、Dynamic、统计原始状态和 typed snapshot
Wrapper: Toasty/SQLx/RBDC/DuckDB/libSQL/HTTP SQL/JDBC Agent、vendor checker/sorter、driver tooling
Metrics: registry、sampler、timeline、Prometheus model、gRPC protocol/runtime
Admin: ingest repository、REST、认证、兼容静态 UI、独立 binary
Facade: stable re-exports and optional features only
```

- [ ] **Step 2: 更新 Toasty 与 driver installer 结论**

所有 `Toasty belongs to druid` 和 `druid-admin owns druid-driver` 当前态结论改为历史，目标态指向 Wrapper；实现状态仍保持当前事实。

- [ ] **Step 3: 更新管理统计归属**

`StatFilter` 和统计状态留在 Core；全局 registry/facade/exporter 移入 Metrics；HTTP/REST service 移入 Admin。

- [ ] **Step 4: 运行归属一致性搜索**

```bash
rg -n "Toasty.*归.*druid|druid-admin.*druid-driver|只允许.*三个" docs/superpowers/specs
```

Expected: matches only in explicitly labelled historical/superseded text.

### Task 3: 更新 README 与计划导航

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/superpowers/README.md`
- Modify: `docs/superpowers/plans/2026-08-12-master-cross-module-roadmap.md`

**Interfaces:**
- Produces: 五 Crate 用户视图和八份专项计划导航

- [ ] **Step 1: 更新模块表和 Mermaid**

README 必须区分“当前源码仍为三 crate”与“批准目标为五 crate”，避免把计划当成已实现事实。

- [ ] **Step 2: 加入专项计划导航**

导航列出本计划及 `druid-core-facade-split`、`druid-wrapper-boundary`、`druid-metrics-runtime`、`druid-metrics-grpc`、`standalone-druid-admin`、`druid-facade-cutover`、`five-crate-verification`。

- [ ] **Step 3: 文档一致性门禁**

```bash
rg -n "只允许.*三个|no fourth module|workspace.*三 crate|三模块架构" \
  README.md README.zh-CN.md docs
```

Expected: every match is inside a historical or superseded section.

- [ ] **Step 4: Markdown 结构检查**

```bash
python3 - <<'PY'
from pathlib import Path
for path in [Path('README.md'), Path('README.zh-CN.md'), *Path('docs').rglob('*.md')]:
    text = path.read_text(encoding='utf-8')
    assert sum(line.startswith('```') for line in text.splitlines()) % 2 == 0, f'unbalanced fence: {path}'
print('markdown fences: ok')
PY
git diff --check
```

Expected: both commands exit 0.

**Suggested commit:** `docs(architecture): define five-crate target topology`
