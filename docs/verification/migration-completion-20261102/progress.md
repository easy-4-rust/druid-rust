# migration-completion-roadmap 执行进展

更新：2026-11-02

## 已完成（12/68）

| 任务 | 证据 |
|---|---|
| 1.1 基线冻结 | `baseline.md`、`baseline-dirty-paths.txt`（SHA-256 `e3f0068a…`） |
| 1.2 `memory-stats` 正式依赖 | `crates/druid-admin/Cargo.toml` 正式依赖段；`cargo check -p druid-admin --all-features` 通过 |
| 1.3 严格 Clippy | 修复 `sql_parse.rs` 未使用 `kw`、Admin 12 个 cast/doc 错误；`cargo clippy -p druid-admin -p druid-rdbc --all-targets --all-features -- -D warnings -A clippy::pedantic` exit 0 |
| 1.4 Filter 回归 | part_01 34 + manager 3 + part_02 6 = **43 passed**；保留 Java 大小写敏感语义 |
| 2.1 Java 用例清单 | `java-test-cases.tsv`：**4759** 个唯一 case ID |
| 2.2 资源清单 | `java-test-assets.tsv`：**822** 个资源、13.46 MiB |
| 2.4 资源精确复制 | 复制到 `crates/druid-test/tests/fixtures/source-java/src/test/resources/`，SHA-256 **822/822** 一致 |
| 3.1 JDBC 方法基线 | `java8-method-evidence-baseline.tsv`，2001 身份，SHA-256 `cfa065c8…`，未评估状态不变 |
| 5.2 Admin 数据合同 | Rust 合同 34 passed（`admin-console-contracts.log`）；前端 5 passed（`admin-web-tests.log`） |
| 5.4 OpenAPI 同源（部分） | 已有路径漂移测试 `admin-openapi-drift.log`：9 passed，但 schema 仍是手写，未达同源生成，保持未完成 |
| 9.1 驱动支持基线 | `driver-profiles-baseline.tsv`：80 profiles，declared 46 / experimental 34 / verified+certified **0** |
| 5.1 Admin 活跃 change | `admin-live-console` 六项任务已勾选、`--strict` 校验通过 |
| 5.3 Admin 三尺寸浏览器 | `browser/`：Mobile 390×884、Tablet 768×1024、Desktop 1280×1024 全路由截图；Basic 登录、空态、部分数据、详情、真实点击复制、慢 SQL、连接/stack 能力态；修复登录跳旧页与 SQL/直方图误标记未上报 |

本轮另完成 Java 1.2.28 oracle 恢复：`D:/druid-oracle/druid-1.2.28`，JDK 17 + Maven Central 独立 settings；`cargo test -p druid-bench --features jdbc-agent --tests -- --include-ignored` 的 **11 个 differential 目标全部通过、0 failed**，覆盖 Callable、ExceptionSorter、JDBC42、MockSorter、Pool、PreparedCache、ResultSet、Statement、Wall，日志 `java-rust-differential.log`。

另：`filter-class-identity-parity` 已通过严格 Clippy 和 `--strict` 校验并归档为 `2026-11-02-filter-class-identity-parity`。

## 当前阻塞

### 1. 全量测试：367/368 目标通过，gRPC 目标环境失败

- 默认 workspace：**4419 passed、5 failed、12 ignored**，唯一失败目标是 `druid-admin --test grpc_network_e2e_test`。
- 全 feature workspace：链接阶段 `0xc0000142` / `0xc000012d`、若干 rlib 缺失，编译未完成；与代码错误不同，需在稳定环境重跑。
- 单独运行 gRPC 目标仍失败；实测测试进程监听 127.0.0.1 端口，但同机外部进程连接该端口 0.5s 超时。WMI 脱离当前 shell 进程树后同样失败，判断为当前 Windows 回环/隔离环境异常，不是产品断言单独可解释的失败。
- 因此 1.5 保持未完成，不能用“其余都绿”替代完整门禁。

### 2. SOURCE_PARITY 有效映射尚未完成

- `docs/migration/source-test-parity.json` 已生成 4759 个用例和 822 个资产记录。
- 当前所有用例仍为 `MISSING/NOT_RUN`；审计确认资产 822/822 精确，但语义映射、目标测试和差分 MATCH 尚缺。
- 禁止按测试名自动映射或伪造 MATCH。

### 3. JDBC 方法证据仍以未评估为主

- 2001 个身份中 capability 仅 25 PARTIAL，real_db 0/2001。
- 3.2–3.7 尚未开始实际方法映射与差分。

### 4. 其他未完成阶段

- Admin 5.5（协议增量可选，需需求确认）；门面收敛 6.x；静态分发 7.x；durable 2PC 8.x；真实驱动 9.2–9.8；Metrics/Admin 生产验证 10.x；最终发布 11.x；规格同步 12.x。

## 下一步

1. 在稳定网络/CI 环境重跑默认、全 feature 测试与文档测试，关闭 1.5。
2. 完成 `exception-sorter-property-parity` 的剩余验收（1.6）。
3. 开始 SOURCE_PARITY 语义映射：按 JDBC 核心对象、Pool/Filter、SQL/Wall 分批，先映射现有真实测试，禁止名称猜测。
4. 3.2 从 Connection/Statement/PreparedStatement/ResultSet 开始，逐方法关联 Java 用例和 MATCH。
5. 5.4 仍需把 OpenAPI schema 与 handler 定义收敛到同源生成；5.5 仅在确认需要 push 丰富字段时另立协议增量。
