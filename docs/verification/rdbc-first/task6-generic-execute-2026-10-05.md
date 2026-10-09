# Task 6 generic execute 行数与子游标关闭验证（2026-10-05）

状态：`IN_PROGRESS / FIX_REQUIRED`。规格事实源仍为 ADR-RDBC-006 与
`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
本记录不关闭 Task 6，不启动 Task 7，不缩减 Task 7–8/P05–P07 范围。

## BASE 行为与实际修正

BASE `5afe860` 的 `prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics`
要求真实缓冲查询通过 generic execute 保留行数，且 Before/After Filter 各进入一次。
新测试把查询扩为两行，避免把固定一行或固定零当作行数实现。

- 新增 SQL-neutral `ResultSet::known_row_count() -> Option<u64>`，默认未知，不请求数据库、
  不推进游标。Toasty 等 common buffered adapter 与 SQLx buffered actor 返回真实 Vec 长度。
- 普通和 Prepared generic summary 使用首个结果的已知行数；查询 update count 仍为 None。
- pooled resource 保留初始已知行数。getter 同时检查 logical lease 与共享 resource；
  more-results 或 Statement cascade 关闭资源后返回 None。
- 普通 Statement 先 CloseAll 子游标，再取得 raw Statement 锁并关闭。
- Prepared logical close 先结束子游标，再恢复属性、清理参数并归还缓存/关闭 raw。
  子游标失败或调用者取消时，raw 尚未转移，registry 继续持有资源责任。

## RED/GREEN 与独立复审

| 场景 | 实际结果 |
| --- | --- |
| 两类 generic execute 行数 | RED 0/2：实际 Some(0)，预期 Some(2)；修正后 2/2 PASS |
| more-results 后 getter | RED 0/2：is_closed=true 仍返回 Some(2)；共享 resource 检查后 2/2 PASS |
| 普通/Prepared close 级联 | 真实 Toasty 首先复现两类 Statement close 后游标仍 open；未知 raw fixture 同样复现；RED 2 PASS/3 FAIL，修正后 5/5 PASS |
| Prepared 缓存复用 | Toasty/SQLx 开启缓存，验证 count=1、再次 prepare hit=1、新游标读取7、旧游标持续关闭 |
| 未知行数 | 受控 raw driver 默认 None；摘要及 proxy open 零次 next，消费者推进后一次；不作为真实 streaming 数据库证据 |
| 关闭失败/取消重试 | 普通 Statement raw cursor 首次 attempts=1/statement closes=0；重试 attempts=2/statement closes=1；重复 close 计数不变 |
| 最终专项 | `task6-generic-row-count-final.log`：8/8 PASS |
| 独立只读复审 | 首次1 Important：closed getter；修正及追加验证复审0 Critical/0 Important/0 Minor |

日志均位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`：
`task6-generic-row-count-red.log`、`task6-generic-row-count-close-red.log`、
`task6-generic-row-count-close-green.log`、`task6-generic-row-count-cascade-red.log`、
`task6-generic-row-count-final.log`。

## 回归与门禁边界

- 三核心默认串行回归：`cargo test -p druid-rdbc -p druid-wrapper -p druid --offline -- --test-threads=1`，
  exit0，198结果块，3343 PASS/0 FAIL，含1个 RDBC doctest。
  日志 `task6-generic-row-count-core-final.log`。该轮专项运行了前6项；后续追加两项
  关闭失败/取消测试由最新8/8专项验证，追加时生产代码没有再修改。
- 前次 broad regression exit101，3087 PASS/2 FAIL，准确暴露关闭 getter；不作为最终通过证据。
- workspace默认测试尚未执行：离线构建因缓存缺少 `anymap3 v1.1.0` exit101。
  日志 `task6-generic-row-count-workspace.log`；不能报告全workspace通过。
- 安装的 stable Rust1.99：不作为指定1.97.1或MSRV1.95证明；既有8条fetch_update弃用警告保留。
- 全部实际修改/新增 Rust27文件 rustfmt check PASS；git diff check PASS。
- final purity PASS，STANDARD_FILES=42。authority PASS：79身份/25 TASK7_PENDING。
- 这次新增标准方法和源码修改尚未刷新全部 API/source/compiler manifests；旧绿色证据不能替代新鲜全门禁。
- C2 `--no-execute` 仍失败于原 BASE generic execute/generated keys 用例缺少 explicit arm。
  本轮专项只是其中行数、关闭语义的真实修正，不把它绑定为整个 BASE 用例完成。
- canonical prepare 仍缺 generated-keys 请求入口，plain/auto/index/name 四种准备方式未恢复；
  具体 Prepared 身份与完整 BASE Filter/key 断言仍待恢复。
- C1 435/479 override-only 方法证据问题仍开。Callable generic summary 的等价行为尚需
  有独立证据后修正；本轮没有宣称三种 Statement family 全部完成。
- 普通 Statement 的关闭失败/取消有本轮证据；Prepared缓存异常、所有资源/owner取消、
  真正异步streaming与真实外部数据库矩阵仍属于后续验收，不扩大本轮结论。
- `druid-driver.rs` 缺失导致的workspace格式门禁与all-feature/质量/最终API门禁仍待关闭。

下一步：补齐 typed generated-keys prepare 请求与真实驱动/缓存传播，恢复完整 BASE 用例，
继续C1/C2门禁和全部 Task6复审；随后按现有计划完成Task7–8与P05–P07。
保持main，未stage/commit/push/publish。
