# CD-01 登录超时合同证据

## 实现与迁移

- `druid-rdbc::data_source::CommonDataSource` 新增 `try_login_timeout() -> Result<Option<Duration>, RdbcError>`。默认实现将旧 `login_timeout()` 的结果包为 `Ok`，使既有外部实现无需立即改写；旧入口标记弃用，新消费者改用 fallible getter。
- `None` 表示实现未显式配置并沿用系统/驱动默认；`Some(Duration::ZERO)` 表示显式无限等待。`set_login_timeout` 保持其已有 `Result` 错误载荷。
- 新增 [登录超时迁移说明](../../../../docs/rdbc/common-data-source-timeout.md)。能观察底层读错误的实现必须覆盖 `try_login_timeout`，不能把错误映射为 None 或默认值。

## TDD 与验证

- RED：先加入 `common_data_source_timeout_contract_test.rs`，执行 `cargo test -p druid-rdbc --test common_data_source_timeout_contract_test --locked --offline`。预期编译失败：`try_login_timeout` 不是 CommonDataSource 成员，外部实现和消费代码均无法编译。
- GREEN：加入带默认适配的 fallible getter 后，同一命令退出码 `0`，2 passed，覆盖 legacy-only 外部实现、None 默认、通过 setter 设置显式零值、`Duration` 最大秒/纳秒值、getter 原始 LoginTimeout 和 setter InvalidArgument 载荷。
- 完整标准合同：`cargo test -p druid-rdbc --locked --offline`，退出码 `0`；所有单元、集成、外部实现、生成合同及 doc tests 通过。
- 跨 crate 消费者编译：`cargo check -p druid-wrapper --features jdbc-agent --locked --offline`，退出码 `0`；`druid-rdbc`、`druid`、`druid-wrapper` 编译通过，验证新增带默认实现的方法没有破坏 wrapper/API 外部实现。
- 格式与规格：`cargo fmt -p druid-rdbc -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` 均退出码 `0`。

## API 与范围说明

- API 变化为新增带默认实现的方法并弃用旧读取入口；对仅实现旧 trait 的消费者保持源码兼容。新增方法只通过标准 `druid-rdbc` 路径公开。
- `druid-rdbc` 当前没有 Java 秒数到 `Duration` 的适配调用路径；本项证明 Rust `Duration` 全范围/精度不截断。未来适配器接入 Java 有符号秒数时须遵循 CD-01 的负值和范围失败合同，不能以本项测试宣称已验证一个不存在的转换路径。
- `api/druid-rdbc.single-standard.after.txt` 是既有快照，尚未包含新方法；本项未安装生成 CLI 或手工伪造快照。自动方法分母、快照重建和失效门禁留在 8.1–8.3。

## 源码身份

| 文件 | SHA-256 |
|---|---|
| `crates/druid-rdbc/src/data_source/common_data_source.rs` | `2A42F1B99C6C166BC9A98FFCCB39F9F265BA85913C4491F7F4217D70C2347A99` |
| `crates/druid-rdbc/tests/common_data_source_timeout_contract_test.rs` | `04EEC37EF7604C6CA833BFE9F39C4FA29EE66FFB913C184071FFC2E27C263BE4` |
| `docs/rdbc/common-data-source-timeout.md` | `65EADEE7132DA6193C00EEF542E6A4F28A1054CA56DEF2766F790443A3DE4C9B` |

本证据关闭 7.1/CD-01；不关闭公共日志合同 7.2–7.3、三类数据源整体迁移文档 7.4 或 API 自动快照门禁 8.1–8.3。
