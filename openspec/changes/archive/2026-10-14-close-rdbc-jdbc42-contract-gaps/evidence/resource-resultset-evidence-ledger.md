# 资源与结果集证据台账（8.8）

本台账针对 8.6 方法矩阵和 8.7 生命周期测试，固定本次验证源码身份、命令、工具版本与后端状态。证据基线 commit：`25d054ff70b397cd325158b76ef0c0056626ec08`。工作树源码 SHA-256 在该 commit 上采集；测试结果均为真实命令退出码，不是计划命令。

## 能力状态

| 区域 | 方法/行为 | 状态 | 限定 |
|---|---|---|---|
| Array | 类型名/类型码、数组值、type map、range 与 ResultSet 游标入口 | 合同通过 | 所需 READ/RANGE/TYPE_MAP/RESULT_SET 位逐项门控；缺位为精确 `0A000`，不下发 driver；具体后端能力不推断 |
| Array | `free`、重复释放、失败/取消重试、子游标与 owner 清理 | 合同通过 | 共享释放状态 + owner registry；子游标显式关闭，父 owner 关闭负责清理 |
| Blob/Clob/NClob | 读/写、range/search/truncate/stream、free | 合同通过 | capability 掩码和句柄生命周期已测；NClob 复用 Clob 合同 |
| SQLXML | 读/写字符串、流、Source/Result、binding readiness、free | 合同通过 | 每种声明能力均有合同测试；表示能力由驱动报告 |
| SQLData | SQL type name、read/write 属性映射、错误保真 | 合同通过 | 应用对象映射合同已测；数据库 UDT 集成未由此证明 |
| RowSet | execute、完整行发布、事件/监听、原始行、accept changes、借用 Connection | 合同通过 | Rust provider 合同通过；标准层没有声称等同于外部 JDBC `javax.sql.RowSet` provider |
| 流式 ResultSet | 首行延迟、`known_row_count=None` 无 poll、EOF/显式 close、关闭错误、owner/statement 级联 | 合同通过 | Rust 测试替身及 SQLx/Toasty feature 测试；不是所有数据库驱动结果集实现的通用保证 |
| 缓冲 ResultSet | 已物化的总行数、游标不因查询摘要而推进 | 合同通过 | SQLx buffered 行集/Toasty 测试；文档明确 SQLx 当前完整读取，不宣称增量网络流 |
| Array/LOB/SQLXML/SQLData/RowSet 真实数据库适配 | 具体方法及能力位 | `NOT_RUN` | 本批没有跑真实数据库资源专项；不可由合同行为或其他功能实测替代 |

未观察到合同实现缺陷。未实现的后端能力属于后端能力状态，保持 `NOT_RUN` 或运行时明确不支持，不自动变成此 change 的新实现范围。

## 实际执行记录

环境：Windows x86_64；`cargo 1.97.1 (c980f4866 2026-06-30)`；`rustc 1.97.1 (8bab26f4f 2026-07-14)`；`Cargo.lock` 固定 SQLx `0.8.6`、Toasty `0.9.0`、Tokio `1.53.1`。所有记录使用 `--locked --offline`。

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `cargo test -p druid-rdbc --locked --offline --test resource_standard_contract_test --test standard_sql_xml_binding_contract_test --test standard_row_set_complete_contract_test --test result_set_stream_test --test task7_shared_result_set_contract_test` | 0 | 32 + 8 + 14 + 5 + 6 = 65 passed |
| `cargo test -p druid --locked --offline --test task7_pooled_async_result_set_test` | 0 | 5 passed |
| `cargo test -p druid-wrapper --locked --offline --features "sqlx toasty-sqlite" --test generic_execute_row_count_test` | 0 | 16 passed |
| `cargo test -p druid-wrapper --locked --offline --features "sqlx toasty-sqlite" --test async_result_set_test` | 0 | 15 passed |
| `cargo test -p druid-rdbc --locked --offline --test standard_sql_mapping_contract_test` | 0 | 11 passed，覆盖 SQLData 外部实现的 UDT 字段顺序、Send-only 对象和错误保真 |
| `cargo test -p druid-rdbc --locked --offline --test resource_standard_contract_test task7_array_value_all_capabilities_gate_before_driver_and_after_free` | 0 | 1 passed，32-mask拒绝路径核对精确 SQLState、重载身份和 driver 未调用 |
| `cargo test -p druid-rdbc --locked --offline --test result_set_stream_test standard_trait_object_stream_is_lazy_send_and_drops_owned_cursor` | 0 | 1 passed，行数探测不 poll 底层游标 |

真实数据库资源专项未执行且不计作通过；相关能力明确留在上表的 `NOT_RUN`。

## SHA-256 身份

以下哈希对应上述基线 commit 的工作树字节：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-rdbc/src/resource/standard_resource_value.rs` | `357796C373B829D71FAF2BDDA29E6472ACEE4856FDE9AE70AD01C454C297ED38` |
| `crates/druid-rdbc/src/resource/rdbc_array.rs` | `5D7910E7CE2124E347E4214F136B240EEAFBBDBE2151761919B1B56425FC6D45` |
| `crates/druid-rdbc/src/resource/rdbc_blob.rs` | `340830C8D423997376E20399809D2409C88E2733C731A3650D4C72CE3255F908` |
| `crates/druid-rdbc/src/resource/rdbc_clob.rs` | `1A4E3FE0034336131F8F9AB29874A24E2AACB5CE8BEA39C08C8F87B00B3BE652` |
| `crates/druid-rdbc/src/resource/rdbc_sql_xml.rs` | `58E7C8C06C37FE062E2238CF01BE3F1086E3A76E0ECA694E6D147F0FD512542F` |
| `crates/druid-rdbc/src/resource/sql_data.rs` | `327855835174584E057B5B191A5DAAD61E75593DB6BE6963358DCFD7677B3674` |
| `crates/druid-rdbc/src/result_set/result_set.rs` | `843858F417C3535DF817892F8173BFE6355D0FF5BA75D8A8AA111F97ACB6E312` |
| `crates/druid-rdbc/src/extensions/row_set.rs` | `B4C4F932DDE13C856202C8125999566D9615CD55EA95EA516E134BA9467438EE` |
| `crates/druid-rdbc/src/extensions/shared_row_set.rs` | `A11A50CE95E987319F8B33BD9BF9784BE9DDD478CBF52E8F9843FB4A7E66B09D` |
| `crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs` | `D4C947CC01E12D0C3529102319CD565B30696EA7B2B36FDB448B610437797F80` |
| `crates/druid/src/core/druid_resource_owner.rs` | `04EC443264762A3E79EDACA596A4E6BE5B2BDC5C2D7C9D996241F960EBFDA9C7` |
| `crates/druid/src/core/druid_pooled_result_set.rs` | `64944A6475FCC104997EE528FE9E70FF79171F7DF331CD2C38083D3592E1FF85` |
| `crates/druid-rdbc/tests/resource_standard_support/array_contract.rs` | `972ABE3DC5336D0535C8D6B1AD9F8475640F343F6D491CBB59035196B08E10DF` |
| `crates/druid-rdbc/tests/resource_standard_support/blob_contract.rs` | `473C891B025B9FB3D537D62AD710D47CA50D2ABD554189E5D69D6C6ED7F58E45` |
| `crates/druid-rdbc/tests/resource_standard_support/clob_contract.rs` | `EA409BBD67BA50DB5645377780C44D664B71EC114E712B2AF86A80882053C749` |
| `crates/druid-rdbc/tests/resource_standard_support/sql_xml_contract.rs` | `88C41C0968757078B4FB068388FC77EF1786B215FEE1C5915F78BF75544C65BA` |
| `crates/druid-rdbc/tests/standard_sql_mapping_contract_test.rs` | `6B211DFAF51C11CC42F98C9662C2719FAAC57C67E799DDF9D79BD4F703E9B511` |
| `crates/druid-rdbc/tests/result_set_stream_test.rs` | `BA6A9150490AEA77FD92F4D3C7ADB1633F0C561819C807C4CA83731F38967F85` |
| `crates/druid-rdbc/tests/task7_shared_result_set_contract_test.rs` | `F3B58F6A815A9BD15A68981EB16921C2632338D5808E7BB4C7207BA2B3510F58` |
| `crates/druid/tests/task7_pooled_async_result_set_test.rs` | `18E324B5F0091C7E03243A22EFE1EFA2B05E1F95FC354139168275D5E1763666` |
| `crates/druid-wrapper/tests/generic_execute_row_count_test.rs` | `4B9A3758039A07BE34C9E9B28DDE2643A2F951CA089A1870F45226C8F427590A` |
| `crates/druid-wrapper/tests/async_result_set_test.rs` | `37C17F78735F7B5CB4046105D6831E828B81C8782E54FA1CEB7227AD2BDB47D4` |

本台账与 [有限方法矩阵](resource-resultset-method-matrix.md) 和 [生命周期测试证据](resource-resultset-lifecycle.md) 一起构成 8.6–8.8 证据边界；三者均不宣称未运行的真实后端通过。
