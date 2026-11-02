# migration-completion-roadmap 基线（2026-11-02）

## 仓库与工具链

| 项 | 值 |
|---|---|
| Java 仓库 | `E:/workspaces/workspace-github/druid` |
| Java HEAD | `fa8dc9912637a2f729eef9f55356621fec18d40e`（`1.2.29-SNAPSHOT`） |
| Java 未跟踪项 | 仅 CodeGraph 索引 `.codegraph/`，未修改源码 |
| Rust 仓库 | `E:/workspaces/workspace-github-easy-4-rust/druid-rust` |
| Rust HEAD | `ed161654c203f60e87ea11b4524ca57496a10e73` |
| Rust 工作区 | dirty；67 个已跟踪/未跟踪路径，快照见 [baseline-dirty-paths.txt](baseline-dirty-paths.txt)，SHA-256 `e3f0068a98bba54e10b8719e89332b2982614794b8411234608a2ac4d975ee90` |
| rustc / cargo | `1.97.1`（MSRV 1.95，门禁须用 `rustup run 1.95` 实跑） |
| 允许并行修改 | 是；本路线保留 Admin、Wall、Filter、ExceptionSorter 的并行工作，不覆盖 |

工作区在审计与实施期间持续有并行修改，本文件是起点快照，不是最终验收基线。后续每批必须先记录新的 SHA/工作区哈希，再运行对应门禁。

## 当前 CI

`main` 最新 run `41316087270`（`ed161654`）结论 failure：

- `rust-contracts`：严格 Clippy 失败，已知 `sql_parse.rs` 的 `kw` 未使用。
- Windows/macOS/Ubuntu 的 `cargo test --workspace`：Filter 类身份旧断言仍期待大小写不敏感。
- 当前工作区已出现 Filter 断言修正（`task6_filter_chain_extended_test_parity_part_01.rs`），但尚无新 CI 证据。

## 当前覆盖率证据

- `druid-admin` 全 feature 独立覆盖率实测成功：**75.95% lines / 75.52% functions / 77.76% regions**。
- 全仓全 feature `cargo llvm-cov` 已多次实际尝试，均未产生全仓报告：
  - 首次 `rustc-LLVM ERROR: out of memory`；
  - 后续独立目标目录出现路径错误；
  - 最近一次发现生产代码使用 dev-only `memory-stats` 编译失败。该依赖现已在工作区移入正式依赖，需重新验证。
- 上述结果只能作为 scoped 信号，不能代替全仓覆盖率门禁。

## 当前迁移事实

| 项 | 当前值 | 来源 |
|---|---:|---|
| 驱动 profile | 80（declared 46、experimental 34、verified/certified 0） | `crates/druid-wrapper/assets/database-drivers.manifest.json` |
| JDBC 方法身份 | 2001，其中 PARTIAL 25、未评估 1976 | `docs/verification/rdbc-first/java8-method-evidence.tsv` |
| 真实数据库证据 | 0/2001 assessed | 同上 `real_db` 列 |
| Java `@Test` 分母 | 4759（参数化/动态尚未展开） | `audit_migration_tests.py` 当前输出 |
| 源测试映射 | 0/4759 | 同上 |
| 测试资源 | 822，精确复制校验 0/822 | 同上 |
| 结构红线 | 无 `todo!/unimplemented!`；2 个已批准超 800 行单 trait 例外；95 个通配导入候选 | 当前源码扫描 |

## 活跃 OpenSpec change

| Change | 任务状态 | 说明 |
|---|---|---|
| `migration-completion-roadmap` | 0/68 | 本路线 |
| `admin-live-console` | 0/6 | 代码已有并行实现，规格尚未收口 |
| `exception-sorter-property-parity` | 6/7 | 严格 Clippy 与完整验收门禁待闭合 |
| `filter-class-identity-parity` | 3/5 | 严格 Clippy 与规格同步待闭合 |

三个现有 change 均通过 `openspec validate`。

## 本轮验收约束

- 不把历史绿灯、同名类型、测试总数、局部覆盖率或 manifest 登记当完成。
- `ignored/NOT_RUN` 不计通过。
- 并行修改必须保留；发生文件冲突时以当前磁盘代码为准，重跑受影响门禁。
- 未获得当前源码证据前，项目保持 `NOT_READY`。
