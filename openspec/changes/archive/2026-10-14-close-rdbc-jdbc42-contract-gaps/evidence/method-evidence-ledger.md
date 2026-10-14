# 8.2：Java 方法到 Rust 与分层证据台账

## 台账与门禁

`docs/verification/rdbc-first/java8-method-evidence.tsv` 以 8.1 固定的 2,001 个唯一 Java 身份为完整分母。每行保留 Rust 类型/方法入口、OpenSpec 场景、批次和后端，以及 declaration、contract、adapter、logical proxy、real DB 五层状态。未核验记录显式为 `UNASSESSED`，Rust/场景入口为 `UNMAPPED`，不会把“有类型”推断成方法支持。

`scripts/verify_java8_method_evidence.py` 是可重复运行的 fail-closed 门禁：先核对台账和 Java 清单身份集合完全相等且无重复；`SUPPORTED` 必须具备精确 Rust 方法映射、有效正向测试引用和至少一层 PASS；`UNSUPPORTED` 必须有有效负向测试引用及精确可观察错误（只有 SQLState `0A000` 不够）；`NOT_RUN` 必须记录阻断原因；所有引用必须指向现存文件/测试函数。

当前门禁报告 `inventory=2001 ledger=2001 linked=9`。九条链接覆盖驱动协商、时间值、取消适配器、池事件、XA 标准合同、公共数据源及原生池 owner 的已运行证据；状态均标为 `PARTIAL`，只表示引用层验证通过，未验证层保留 `UNASSESSED`。其余 1,992 条为 `UNASSESSED`，不能宣称完整 API 一致或 JDBC 合规。XA recovery 的真实 PostgreSQL 行为仍由独立 XA 证据保管；本 Java `javax.sql` 分母不包含 `javax.transaction.xa`。

## 负向门禁

新增 `tools/java8-api-inventory/test_method_evidence_gate.py`，通过临时缩小分母运行四个控制：缺正向证据的 `SUPPORTED` 被拒绝；没有负向引用的 `UNSUPPORTED` 被拒绝；仅有 `0A000`、没有精确观察结果也被拒绝；合法且显式未评估的完整分母通过。测试还经主门禁验证当前全量台账的身份完整性和所有引用。

## 实际验证

| 命令 | 结果 |
|---|---|
| `python scripts/generate_java8_api_method_inventory.py` | 成功；固定 OpenJDK 8u502-b07 SHA-256 校验通过；69 个源文件、2,001 条记录 |
| `python -m unittest discover -s tools/java8-api-inventory -p 'test_*.py' -v` | 5 passed，含 doclet 身份回归和四项门禁控制 |
| `python scripts/verify_java8_method_evidence.py` | `EVIDENCE_GATE_PASS inventory=2001 ledger=2001 linked=9` |
| `cargo test -p druid-rdbc --test driver_registry_negotiation_contract_test --test standard_event_contract_test --test jdbc_temporal_value_contract_test --test xa_standard_external_consumer --locked --offline` | 24 passed |
| `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline independent_cancel_interrupts_running_sqlite_update` | 1 passed，SQLite 执行中断 |
| `cargo test -p druid-wrapper --features jdbc-agent --test common_data_source_public_examples_test --locked --offline` | 1 passed |
| `cargo test -p druid --test standard_pooled_connection_owner_test --locked --offline` | 2 passed |
| `python -m py_compile scripts/verify_java8_method_evidence.py tools/java8-api-inventory/test_method_evidence_gate.py` | 退出码 0 |

公共数据源示例第一次以缺少 `jdbc-agent` feature 运行而编译失败；按 Cargo target 声明补上该 feature 后通过。该失败是命令配置错误，不计作行为失败。

## 源码身份

以下 SHA-256 将本证据绑定到生成器、清单、台账和被引用测试：

```text
tools/java8-api-inventory/JdbcApiInventoryDoclet.java BC33CB1DEC4E70769C711891B74C5F954CA31C3CD529ADA633C283FE1469C830
tools/java8-api-inventory/test_inventory_doclet.py 7EBAA1750DF7E8819636B32B7B7B8D4B5FC08CCD1BE92BF2802117D90E24769D
tools/java8-api-inventory/test_method_evidence_gate.py 2855E648BD4157BA8C165AA5B0A48F1CF1AC7D6B31C6759C9188B7F95EB2961B
scripts/verify_java8_method_evidence.py 2842D0DFE6D372E976053B60B4D53C7E97D6F77CAAB4053713C58068058CB390
docs/verification/rdbc-first/java8-public-method-inventory.tsv 27F424D0FC4262D8862F93044BD61D35A5BAB3FD85511D58D5CCE5DEB7FFE4F5
docs/verification/rdbc-first/java8-method-evidence.tsv C03DF968F1E01808997198D666C9150588447E312A56DDF6F3FD7654D1E32FE7
crates/druid-rdbc/tests/driver_registry_negotiation_contract_test.rs 2362A7F0F4DE50E6A52B8D91A3D56F308848FC7C072CC5334DFEC1836222E770
crates/druid-rdbc/tests/standard_event_contract_test.rs BFEBE712E5BBC4536957BE2C353183CEE780164B92ECC03F8BDE4845CBF4D071
crates/druid-rdbc/tests/jdbc_temporal_value_contract_test.rs FB8371B6896EA767E095F451DABC153F1F95001DF3E127E5413C4063F1B85542
crates/druid-rdbc/tests/xa_standard_external_consumer.rs FBC8D78C156925F21452F8B149B6ADADC3067517E0821FE85C3189FF36B46E6F
crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs 94CB49839C93C0ECFE575DCBDCB3CBCF9E3236A81EF4A76EC3A7E4C57604FBEC
crates/druid-wrapper/tests/common_data_source_public_examples_test.rs 0D109EEC9D6A0EE586293769B120B4183FADDCCE91C515C08B4CD81924FE9C40
crates/druid/tests/standard_pooled_connection_owner_test.rs FD67E2575A7A30774BA419F1E60EC6259BC246679108E089D265014D4D2B5234
```

本任务关闭方法级证据关联和门禁机制，不关闭全部 Java API 方法实现、8.3 源码漂移负向验收、资源结果集矩阵或最终验收。
