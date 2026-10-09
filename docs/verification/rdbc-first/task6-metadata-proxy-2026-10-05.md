# Task 6：真实 Metadata Proxy 合同恢复（2026-10-05）

## 来源与范围

规格：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md`。
固定 BASE：`5afe8609b177544ff7d18461aa84f69efe8ee90e`。
原案例：`druid_pooled_result_set_differential_test.rs#meta_data_proxy_returns_proxy_with_attributes`。
原行为是建立真实 SQLite 五行表，执行 `SELECT id, name FROM t`，获取结果集返回的代理，断言属性数量为零，然后关闭结果集。

## 实现

标准 Metadata getter 委托私有 `meta_data_proxy_core`，保留原来的开放状态检查、Filter hook、policy 错误分类和资源属性存储。具体代理继续保持内部可见，不加入 RDBC 公共接口。

私有诊断测试执行相同真实 SQL 和初始化数据，使用标准 Statement、资源准入、registry 和池化结果集。除 BASE 初始属性断言外，还验证两个 getter 返回的代理共享属性、清除属性同步可见、两列描述以及关闭后 getter 返回 ConnectionDiscarded。

迁移行改为明确的私有 lib 测试；删除原矩阵中的泛化占位案例。原始 1,283 项分母和 BASE 摘要保持不变，目录现为 1,076 exact / 207 parameterized；这不是完成率。

## 验证证据

日志目录：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

- `task6-metadata-proxy-initial.log`：新增测试调用私有 getter 编译失败 E0599，属于编译缺口，不声称运行时 RED。
- `task6-metadata-proxy-green.log`：真实私有诊断测试 12/12 PASS。
- `task6-metadata-proxy-final.log`：lib 71、Metadata ownership 3、结果集矩阵 5，共 79/79 PASS，退出 0。
- verifier 自测 15/15 PASS；final purity 43 个文件检查通过。
- 初次台账写入 CRLF 被严格 LF 检查拒绝；字节写入 LF 后同步反向索引与派生文档，authority 79 行检查通过，Task7 pending25 / physical73，SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`。
- `task6-integer-label-workspace-serial.log`：上一版源码全量退出101，240结果块，3676PASS/2FAIL/0ignored，含2doctests；仅 Callable full-error helper/matrix prepared5vs6。这一结果不覆盖本次 getter 提取。
- 当前 C2：`task6-metadata-proxy-c2-final.log`，已启动，需以终态结论为准。
- 当前全量：`task6-metadata-proxy-workspace.log`，已启动，需以终态结论为准。

## 剩余范围

Task6 仍 FIX_REQUIRED，C1/C2 完整合同、Callable5vs6、调用形式身份、typed index owner、API/SemVer/all-features/MSRV/最终审查尚未关闭。Task7仍8组件，Task8和P05-P07尚未完成。没有创建分支、提交、推送、发布或安装工具。

Task6 metadata proxy C2 terminal 2026-10-05: session81069 exit1, task6-metadata-proxy-c2-final.log advances read_string_length_increments_on_string_get explicitarm. Metadata proxy exact private binding passed this static phase; whole C2 remainsFAIL. Supersedes LIVE81069. Workspace18117 remains live, no current fullsource completion claim.

Metadata fullworkspace terminal：session18117 exit101，240结果块，3677PASS/2FAIL/0ignored，含2doctests。仅 Callable full-error helper/matrix prepared5vs6；supersedes LIVE18117。日志 task6-metadata-proxy-workspace.log。后续字符串修复不由此旧结果证明。
