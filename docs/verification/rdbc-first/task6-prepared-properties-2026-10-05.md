# Task 6 预编译参数、属性恢复与失败清理验证（2026-10-05）

状态：`IN_PROGRESS / FIX_REQUIRED`。本记录不表示 Task 6 完成，也不启动 Task 7。
规格事实源仍为 ADR-RDBC-006 和 `2026-08-24-rdbc-single-standard-unification` 计划。

## 实际恢复的行为

- BASE `prepared_bound_setters_execute_and_batch_against_real_sqlite`：恢复 Toasty/SQLite
  十五类标量 setter、覆盖参数、清参后拒绝执行、批次快照、query/eager/generic 返回值、
  参数间隙拒绝、BinaryStream 延迟读取及真实落库。
- 私有 adapter parameter state 保留完整 `PreparedInputParameter`，绑定和加入批次时不读取流。
  执行边界 materialize 支持 BASE 的流、Reader 和异步 LOB；不将不支持的资源伪造成 NULL。
- pooled Filter 保留完整 prepared descriptor；仅原生参数不能生成 scalar view 时使用空的 scalar view。
- 部分批次执行失败和执行中取消消费本次 raw/逻辑批次，避免重试已消费流。
  在 BeforeFilter 阶段取消则保留尚未开始的批次，支持安全重试。
- BASE 空流/空 Reader 的十八种绑定恢复为成功，并逐项执行真实 SQLite INSERT，
  验证十八行的自增 ID、SQL NULL 和浮点值。
- BASE 属性恢复专用用例替代泛化循环：保存底层初始属性，按 max-field-size、max-rows、
  timeout、fetch-direction、fetch-size 顺序恢复；首错立即停止，重试跳过成功部分。
  恢复失败使句柄不再缓存，随后 prepare 必须创建新的 raw handle。
- 创建时属性读取错误在 registry 注册后清理，保留原错误；资源实际成功关闭时计 prepared
  关闭统计，缓存命中清理另计 delete。关闭失败保留责任，重试成功计一次。

## RED/GREEN 与复审

| 场景 | 实际证据 |
| --- | --- |
| 空流迁移预期 | 原测试要求 Unsupported 而 BASE `.unwrap()`；恢复成功绑定，真实 SQLite 十八行；目标文件 11/11 PASS |
| 初始属性捕获 | 新 BASE 专用用例 RED：getter events 为 `[]`；修正后精确 getter/恢复/重试顺序与 prepare-count 1→2 PASS |
| getter 失败关闭统计 | 专门测试 RED：closed-count 0，预期 1；修正后首次创建和缓存命中均 PASS |
| cleanup 重试 | 首次 raw close 失败时计数 0；connection close 重试成功计数 1，重复 close 仍为 1 |
| 属性 matrix 与 getter/cleanup | `task6-property-capture-green.log`：3/3 PASS |
| Filter/stat/cleanup 回归 | `task6-property-policy-regression.log`：21 + 7 = 28/28 PASS |
| 参数取消验证 | wrapper lib 8/8；`prepared_resource_execution_test` 3/3 PASS |
| Python Task 6 验证器自测 | `task6-python-current.log`：37/37 PASS；不能代替生产行为门禁 |
| 标准纯度 | final PASS，STANDARD_FILES=42 |
| Rust 格式 | 最终 21 个实际修改/新增 Rust 文件检查 PASS |
| 两核心默认回归 | `task6-property-final-regression-3.log`：exit 0，185 个结果块，2875 PASS / 0 FAIL，单线程，doctests 0 |
| 独立只读复审 | 第一次 1 Important：getter 失败漏计 prepared stats；修正后复审 0 Critical / 0 Important / 0 Minor |

运行日志位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。
本轮使用已安装 stable Rust 1.99，不作为 1.95 MSRV 或指定 1.97.1 工具链证明。

## 全量门禁和剩余事项

- 属性修改前单线程两核心 crate 默认回归：185 个结果块，2873 PASS / 0 FAIL。
  并行执行曾失败于共享全局统计开关测试，不能把单线程 PASS 表述为并行稳定。
- 属性修改后前两次 broad regression 暴露 timeout/cleanup 迁移预期与 BASE 不一致；
  已恢复 BASE 预期并通过对应 28 项测试。最终第三次完整回归 exit 0，2875 PASS / 0 FAIL。
- C2 `--no-execute` 已越过属性恢复条目，仍 FAIL 于缺少 explicit case arm 的
  `prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics`。
  其余未恢复分支仍不得以通用 SQLite 循环作为 BASE 行为证明。
- C1 435/479 generated override-only 方法证据仍需替换，不能将它们当作真实默认/adapter 行为。
- 新生产源码使已冻结的 source/hash/API/compiler 证据需要重新生成与核验；未声明当前证据全绿。
- `cargo fmt --all -- --check` 未通过：Cargo 声明
  `crates/druid-wrapper/src/bin/druid-driver.rs`，但该文件既不存在，也不在当前 tracked 文件中。
  `managed-driver-install` 对应交付仍需完成；本轮未删除声明绕过检查。
- stable 缺少 Clippy；1.97.1 工具链缺少 rustc/rustdoc。未安装工具，质量/MSRV 门禁未关闭。
- `verify_no_physical_standard.py` 在 UTF-8、stable/offline 环境下仍 exit 1：public-api
  请求 `nightly-2026-08-07-aarch64-apple-darwin`，rustup 创建临时文件被拒绝；
  API 门禁未取得当前成功证据。未安装该工具链，也未把静态文本检查作为 API 证明。
- `verify_task6_migration_authority.py` PASS：79 身份，25 TASK7_PENDING；SHA-256
  `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`。
- Task 6 复审整体、Task 7–8、P05–P07，以及真实外部数据库验证均未完成。

Ruling：BASE 实际行为是迁移兼容性依据，纠正迁移测试的空流拒绝、timeout 不恢复和
清参顺序预期；保留全部 fatal 分类、首错停止和 exactly-once 关闭断言，不降低验收标准。
本轮保持 main，未 stage/commit、未推送或发布。
