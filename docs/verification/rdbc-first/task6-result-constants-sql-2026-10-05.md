# Task6：结果集常量、SQL 来源与初始流计数恢复（2026-10-05）

## 来源与实际变更

规格和计划沿用 single-standard-unification；固定 BASE 为 `5afe8609b177544ff7d18461aa84f69efe8ee90e`，三个原测试来自 druid_pooled_result_set_differential_test.rs。

result_set_constants_match_java：完整恢复 FETCH_FORWARD/REVERSE/UNKNOWN=1000/1001/1002，TYPE_FORWARD_ONLY/SCROLL_INSENSITIVE/SCROLL_SENSITIVE=1003/1004/1005，CONCUR_READ_ONLY/UPDATABLE=1007/1008，HOLD_CURSORS_OVER_COMMIT/CLOSE_CURSORS_AT_COMMIT=1/2。原常量名和十个断言全部保留，独立测试绑定真实生产常量。初始测试产生10个E0599，属于编译缺口，不宣称运行时RED。

sql_returns_some：真实SQLite五行表和标准Statement/registry装配不预先执行查询；测试调用实际池化Statement.execute_query("SELECT * FROM t")。生产openhook通过ResultSetFilterContext::sql getter捕获SQL，保留原Some/containsSELECT断言和关闭，并检查实际statement.last_sql。上下文SQL仍位于Druid内部，未加入RDBC公共契约。没有测试直接设置SQL字段或通过独立快照代替真实执行。

stream_reader_counts_zero_initially：真实标准游标，保留InputStream与Reader初始计数都为0及关闭。

原诊断fixture分为真实Statement建表装配与原始驱动游标准入两步，现有诊断案例仍使用真实驱动、相同registry/lease/policy，不添加未执行行为的替身。

## 验证

日志位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

- task6-result-constants-initial.log：编译失败，10个缺失常量错误。
- task6-result-constants-sql-green.log：首次105项回归PASS。
- task6-result-constants-sql-final.log：最终105/105PASS，退出0；Druid lib75、FilterStat21、Metadata ownership3、结果集矩阵6。
- task6-result-constants-sql-c2.log：退出1，下一处 all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity 缺少明确分支。整体C2仍FAIL。
- verifier自测15/15PASS，final purity检查43文件；authority79行、pending25、physical73，固定SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`；格式与diff检查通过。

三个映射改为明确独立测试，清除原矩阵泛化占位。原1,283分母和BASE摘要不变，现为1,080exact/203parameterized；尚缺明确字面量分支64：buffered26、lease2、recycle12、Filter24。明确分支存在也不代表完整行为认证。

## 剩余范围

当前105项是本次相关回归，未重新执行字符串/常量改动后的全Workspace。旧metadata版3677PASS/2FAIL属于此前源码。新增标准默认字符串getter及恢复常量仍需最终API/SemVer门禁。

Task6仍FIX_REQUIRED，C1/C2、Callable原始prepared5vs6、owner/调用形式身份、all-features/MSRV及最终审查未关闭。Task7剩8组件，Task8和P05-P07仍属于完整目标。本轮所有运行句柄已终态；未创建分支、提交、推送、发布或安装工具。
