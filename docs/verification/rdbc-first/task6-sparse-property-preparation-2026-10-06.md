# Task6 稀疏属性默认合同准备（2026-10-06）

此文为当前完整workspace运行期间的只读源码核查，不是实现或运行验收。

固定 BASE5afe8609b177544ff7d18461aa84f69efe8ee90e 原 sparse_result_set_property_defaults_report_exact_capabilities，body SHA ef39cee737f247494ff796bf936516d476425a373b42b8b0b07d169bafb3d650。原31个 assert_unsupported_operation 调用都必须保留，不以少数属性检查替代。

原 SparsePhysicalResultSet 只有 close/is_closed 实现，31项错误来自真实默认方法。当前 canonical ResultSet 默认 input_stream/reader 会调用 value，因此稀疏对象返回 result_set_value，而原 ASCII/Unicode/Binary/character stream 默认分别直接拒绝，不读取值。默认 result_set_type/concurrency 返回 ForwardOnly/ReadOnly，原默认是各自精确 UnsupportedOperation。默认 row_updated/inserted/deleted 返回false，原默认直接拒绝。默认七个 row mutation 操作当前是 SQL0A000，原默认是精确 Java UnsupportedOperation。

不能通过 SparseDefaultResultSet 覆盖这些方法伪造默认等价。应正式接入原31调用测试取得实际RED，再恢复真实默认，并检查具体驱动能力是否需要明确实现。

CodeGraph确认 BufferedRowSet 已自行实现 input_stream/reader 并保持原字符串/字节/NULL；不依赖 canonical stream conversion。但它只实现 row_updated，row_inserted/deleted 仍继承当前成功false默认，所以恢复标准拒绝时需核对其真实缓冲行为。SQLx当前明确实现 result_set_type/concurrency，但未自行实现stream和row flags；StandardAdapterResultSet同样没有stream、type/concurrency/row flags覆写。不能仅凭“缓冲”假设其基线能力，必须对照旧驱动实现及完整消费者运行证明。

完整31调用草案保存 .superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/sparse_property_full_contract.rs.txt；不是Cargo测试目标。只有 mut/typed ColumnIndex/FetchDirection 和 helper名称的必要映射，原参数id、列1、方向1000、fetch10及精确错误均保留。当前账本/源码floor1239PASS44FAIL没有因草案改变，Task6/P04其余与完整18顶层任务不关闭。

工具链只读检查：1.97.1和1.99.0可用，1.95未列出；cargo-public-api/cargo-semver-checks未在PATH找到。未安装或升级，不据此宣称API/MSRV门禁通过。

同一完整workspace26074/Cargo4680持续存活，编译子进程推进到Admin/Bench测试。仅写非Cargo文档与草案；不修改Rust/Cargo、不重启回归。终态结果待证，不能用旧3716/2或相关618/0替代。


## 固定 BASE 缓冲行状态能力核查

完整 workspace 同一26074仍LIVE，cargo4680及变化的rustc子进程证明持续编译；本次仅修改此准备记录，不修改Cargo/Rust输入。

只读核对固定BASE5afe8609b177544ff7d18461aa84f69efe8ee90e的 `crates/druid-rdbc/src/spi/rdbc_result_set.rs`：PhysicalResultSet 默认 row_inserted/row_deleted 分别返回精确 UnsupportedOperation(result_set_row_inserted/result_set_row_deleted)，但同文件 RowSetResultSet 分别在1474–1477、1480–1483明确覆盖，先 ensure_open 再返回 false。row_updated 在1468–1471同样明确覆盖。

CodeGraph 当前 BufferedRowSet 完整源码确认：input_stream/reader 已明确覆盖；row_updated 在413–417明确覆盖且检查关闭状态，row_inserted/row_deleted 没有覆盖。因而不能把恢复接口默认拒绝机械应用到具体缓冲能力：下一正式TDD需补充真实 BufferedRowSet 开放时两个false、关闭后精确Other(result set is closed)的控制测试，再补齐实际缓冲覆盖。该控制必须调用生产类型，不通过测试fixture伪造返回。具体缓冲流仍由已有覆盖保留。

此处是来源与实施约束证据，尚非runtime RED/GREEN；不绑定迁移账本，不减少44项或18顶层任务。
