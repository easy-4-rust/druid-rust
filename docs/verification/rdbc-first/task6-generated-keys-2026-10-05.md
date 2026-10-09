# Task 6 generated-keys 准备请求恢复（2026-10-05）

状态：`IN_PROGRESS / FIX_REQUIRED`。继续现有 ADR-RDBC-006、单一标准规格和实施计划；
没有关闭 Task 6，没有启动 Task 7，没有缩减 Task 7–8/P05–P07 范围。

## 已实现的合同切片

- 标准 `Connection::prepare_statement_with_generated_keys(sql, options, GeneratedKeys)`
  合并 Java auto/index/name 准备重载。默认 `None` 委托普通准备；其他模式在创建 raw
  statement 前明确报不支持，避免吞掉请求或创建后遗留句柄。
- Druid pooled prepare 统一进入这一请求路径；模式、列内容、列顺序与结果集选项参与
  私有缓存身份。显式 None 与普通标准准备入口使用同一缓存身份。
- `ExternalConnectionLease` 原样传播请求、选项和底层错误。
- Toasty/SQLx 的 SQLite auto 准备已接入真实生成键执行路径。SQLx owned Prepared
  保存准备请求，generic/update/prepared batch 使用该请求，缓存复用不丢失模式。
- SQLx backend 判断使用 actor-aware `standard_backend_key_core`；连接转入 owned actor
  后不能把 detached shell 的 `connection=None` 误认为 SQLite 模式不受支持。
- SQLx generic/query 的 SQLite `INSERT ... RETURNING` 共用准备查询执行路径，
  保留原结果列与未消费游标，同时更新生成键；纯查询和忽略插入清除旧键。
  AST 判断与真实查询/键读取使用同一 actor 连接锁保护的执行范围。

## BASE 与边界裁定

BASE `5afe860` 的四方式测试实际要求 Toasty/SQLite plain/auto/index/name 返回 id1–4，
但旧 Toasty `prepare_physical_statement` 只把 `key.sql()` 交给句柄，没有处理任意列投影。
本轮恢复标准请求链路与 auto 实际行为；按列请求当前明确拒绝，不能用 last_insert_rowid
冒充任意列选择。完整 BASE 用例及列选择真实投影继续待完成。

SQLx 的普通 None 准备原先没有请求生成键。本轮真实测试保留其空键集行为；
Toasty plain 准备沿用 BASE 返回 id 的行为，不为了统一测试结果改变已有驱动语义。

## 实际 RED/GREEN

日志位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

| 证据 | 结果 |
| --- | --- |
| `task6-generated-keys-interface-red.log` | 缺少标准方法的编译失败；仅是接口缺失证据，不冒充运行时 RED |
| `task6-generated-keys-request-red.log` | 可编译后运行时1 PASS/1 FAIL：pooled auto 请求落入默认不支持 |
| `task6-generated-keys-request-green.log` | 2/2 PASS：默认拒绝无 raw prepare；六模式/列顺序隔离及缓存复用 |
| `task6-generated-keys-lease-red.log` | 运行时0/1：外部租约没有转交底层请求/错误 |
| `task6-generated-keys-lease-green.log` | 1/1 PASS：四模式、列顺序、原 SQL/options/error 原样传播 |
| `task6-generated-keys-sqlite-red.log` | 真实 SQLite0/1：auto 准备入口未实现 |
| `task6-generated-keys-sqlite-diagnostic-red.log` | 中间失败定位 SQLx plain None 键集为空；保留既有行为后又发现 actor 转移后的 backend 判断问题 |
| `task6-generated-keys-sqlite-final.log` | 后续 RETURNING 修复期间覆盖为编译失败；不能再作为先前3/3通过的证据 |
| `task6-generated-keys-returning-red.log` | 真实 SQLite0/1：generic RETURNING 丢失 auto 请求 |
| `task6-generated-keys-returning-green.log` | 4/4 PASS：generic RETURNING 保留键和原结果游标；含既有auto/cache/update/单项batch/拒绝列选择 |
| `task6-generated-keys-query-red.log` | 真实 SQLite0/1：execute_prepared_query 丢失 auto 请求 |
| `task6-generated-keys-query-green.log` | 5/5 PASS：上述专项加查询入口、带注释SQL、忽略插入后同句柄清除旧键 |
| `task6-generated-keys-failed-attempt-red.log` | 真实 SQLite0/1：未读取的上一轮键在下一次唯一约束失败后泄露 |
| `task6-generated-keys-failed-attempt-green.log` | 6/6 PASS：update/query × 唯一约束错误/缺参均清除旧键 |

最后 raw 请求测试增加了非默认 options 的精确传播和同 SQL/mode 不同 options 的缓存隔离；
最新三核心回归实际执行该2/2请求专项与1/1租约单元测试。

## 验证状态

- 前次 `task6-generated-keys-core-final.log` exit101：Windows正在运行的测试文件被并发专项构建锁住（os32）；保留失败记录，不作为通过证据。
- `task6-generated-keys-core-final-2.log` exit0：200结果块/3353 PASS/0 FAIL，含1doctest；发生在后续failed-attempt reset修正之前，不能替代该修正的新鲜回归。
- reset修正后wrapper默认串行回归 `task6-generated-keys-wrapper-final.log` exit0：
  42结果块/278 PASS/0 FAIL，doctests0，包含最新生成键6/6。
- 生成键切片10个Rust文件rustfmt check通过；本轮后续修改3文件重新check与diff check通过。
- standard purity final PASS：42文件。authority PASS：79身份/25 Task7 pending。
- 独立只读复审：0 Critical / 5 Important / 0 Minor，整体仍FIX_REQUIRED。
  Finding4 SQLx prepared直接失败部分经RED/GREEN与复查已修正；等待取消由控制流证明，
  尚无新的取消运行测试。Toasty/ordinary/batch部分执行的结果状态不计入已完成。
  仍open：DO UPDATE RETURNING历史键、零行/非插入的update/batch历史键、缓存交接旧键、
  Toasty RETURNING/单项batch不保留auto键。未宣称REVIEW CLEAR。
- C2 complete generic/generated-keys explicit arm、C1 435/479 override-only 证据继续未完成。
- 新标准方法/源码尚未刷新全 API、source/compiler manifests；当前门禁不得引用旧绿色
  快照作为新实现完成证明。all-feature/MSRV/指定工具链/Clippy/完整workspace仍未闭环。
- 本轮使用已安装 stable Rust1.99；不能作为MSRV1.95或指定1.97.1验证。
- 未 stage/commit/push/publish/install。

批量专项只执行一个batch元素，不能证明多项生成键集合、顺序或部分失败行为；
当前SQLx单键状态仍只保存最后一个键，此项继续待完整BASE/批量验收。

Ruling：wrapper在AST判断中显式依赖现有workspace `sqlparser`，Cargo.lock只增加该
包的依赖边；没有安装/升级库。不得通过SQL字符串前缀或解析后的字符串猜测INSERT，
否则注释、CTE和结果列顺序会使生成键判断不可靠。P05依赖隔离义务保持不变。

下一步：完整按列准备与真实投影、具体 Prepared 身份及 BASE Filter/key/disposition 断言；
继续 C1/C2 和 Task6完整复审，再按计划完成 Task7–8/P05–P07。

## 后续 Toasty 与完整 prepared batch 修复

本节更新前述较早的 6/6 验证与部分 open 项，Task 6 整体仍为 `FIX_REQUIRED`。

- 前轮租约与执行结果专项实际为运行时 RED 6 PASS/2 FAIL → GREEN 8/8：
  零行插入、UPDATE/DELETE 不返回连接历史键；缓存中新逻辑租约在尚未执行时读取空键集。
  最新日志为 `task6-generated-keys-lease-outcome-red.log` 与 `...-green.log`。
- 扩展同一真实 SQLite 合同到 Toasty，运行时 RED 8 PASS/4 FAIL → GREEN 12/12。
  修复 generic INSERT RETURNING 被误送 update 路径、query 丢失键、prepared update
  缺少当前更新计数、执行失败后旧键泄露。查询结果与键在 OwnedAdapterExecutor 的同一
  连接锁范围内取得，读取生成键不消费原 RETURNING 游标。
- `StandardQueryResult` 新增 wrapper 私有查询生成键字段，默认 None；其他驱动查询保持
  既有结果。Toasty SQLite 使用 AST 判断 INSERT RETURNING，不猜测结果第一列。
- 多项 batch 运行时 RED 12 PASS/2 FAIL → GREEN 14/14。Toasty/SQLx 先在局部变量
  按命令顺序保存全部键，整批成功后发布；部分失败只在 BatchUpdateException 保留已执行
  项的无损计数，句柄不发布部分键/旧键。失败队列已消费，下一次空批次不重放。
  真实数据库同时证明第三个失败后的命令没有执行、已成功的命令确实保留在数据库中。
- 日志：`task6-toasty-generated-keys-red.log`（8/4）、`...-green.log`（12/0）；
  `task6-batch-generated-keys-red.log`（12/2）、`...-green.log`（14/0）。
- 当前仍未完成：DO UPDATE 的准确键身份、任意列真实投影、多行单条 INSERT 的完整键集、
  全 BASE 身份/Filter/disposition 断言、C1/C2 真实行为证据、最新 API/compiler/feature/MSRV
  门禁及独立复审。
- 随后增加 `cancelled_second_batch_item_does_not_publish_partial_or_previous_keys` 运行测试：
  在保留上一轮成功键、当前批次第一项完成、第二项进入驱动等待时取消；通过生产 prepared、
  executor 和参数材料化证明旧键/部分键不发布、更新计数失效、队列不重放。测试只注入
  可控驱动等待，不冒充真实 SQLx 网络取消、驱动回滚或全部 Task7 资源取消合同。
  该补充是对已实现发布边界的运行时验证，不登记为新的 RED/GREEN 修复。

最新验证：

- 三个核心 crate 默认串行回归 `task6-toasty-batch-core-final.log`：exit0，200 结果块，
  3362 PASS/0 FAIL，含1 doctest；已覆盖本轮全部生产行为修改。
- 补充取消测试后 wrapper 完整默认回归 `task6-toasty-batch-wrapper-final.log`：exit0，
  42 结果块，287 PASS/0 FAIL，含生成键14/14与取消测试1/1。
- `cargo test --workspace -- --test-threads=1` 在离线依赖解析阶段 exit101，缺少
  `anymap3 v1.1.0`；没有运行完整 workspace 测试，没有安装依赖。
- 7个修改Rust文件 rustfmt check与默认配置 `git diff --check` exit0；standard purity
  final42与authority79/pending25再次PASS。
- C2 `--no-execute` 仍exit1：完整 generic/generated-keys 参数化测试缺少显式case arm。
  不放宽验证器、不填充无意义断言，完整BASE验收义务保持不变。
- 本轮仍使用已安装 stable Rust1.99；all-feature、MSRV1.95、指定1.97.1及API/compiler
  最终门禁未闭环。现有Rust1.99废弃警告仍在日志中，不宣称无警告或Clippy通过。

## INSERT RETURNING 当前行键身份与错误语义修复

本节更新此前 RETURNING upsert 与多行返回的 open 项；Task 6 整体仍为 `FIX_REQUIRED`。

- 真实 SQLite 的 Toasty/SQLx 合同复现 upsert 更新 id=1 却返回历史 id=2：
  `task6-returning-current-row-red.log` 为14 PASS/2 FAIL，修复后16/16。
- 新增私有 `SqliteReturningKeyPlan`，读取实际 schema、列与主键索引，追加并移除
  私有 RETURNING 键列；原用户列、游标与行顺序保留，生成键按当前返回行完整收集。
  metadata 与执行位于原 owned executor/actor 连接锁范围，未另建连接。
- 用真实 SQLite 验证 INTEGER PRIMARY KEY DESC、WITHOUT ROWID、三个 rowid
  名称全部被用户列遮蔽、表/列名含引号等边界；不从用户第一列猜键，不取连接历史键。
- prepare 后 DROP 表的回归实际 RED17 PASS/1 FAIL，当前重新复现日志为
  `task6-returning-schema-resume-red.log`。元数据未找到表时执行原 SQL，保留
  驱动 SQL 异常类别；`task6-returning-schema-green.log` 为18/18。
- 首次核心回归还发现辅助解析把无效查询改为 SqlParseError；已有真实 Toasty
  pooled Statement 用例复现 `query_error.sql_exception().is_some()` 失败。
  辅助解析失败或表名无法投影时交回原驱动；`task6-returning-error-green.log`
  相关7/7 PASS。首次失败保留在 `task6-returning-core-final.log`，不作通过证据。
- 删除未再调用的 SQLx 历史键查询 helper；StandardQueryResult 私有键字段现在为
  Vec，默认空集合。该内部变化不等于按任意列生成键或真实流式读取已完成。
- fresh standard purity final42与authority79/pending25 PASS；C2 no-execute仍失败
  于完整 generic/generated-keys explicit case arm，没有放宽门禁。
- 仍未完成：无 RETURNING 的 prepared update/upsert 和多行单条 INSERT 完整键集、
  列名/列序号真实投影、完整 BASE 身份/Filter/disposition、C1/C2、最新 API/compiler/
  all-feature/MSRV及全Task6复审。Task7–8/P05–P07全部保持原目标范围。

本节最终验证：

- `task6-returning-core-final-green.log`：三个核心 crate 默认串行完整回归 exit0，
  200结果块，3367 PASS/0 FAIL，含1 doctest与生成键18/18。
- `task6-returning-workspace-final.log`：workspace离线验证exit101，缺少anymap3
  v1.1.0，未运行完整workspace测试，未安装依赖。
- `task6-returning-verifier-tests.log`：迁移校验器自身37/37 PASS。
- 9个修改Rust文件rustfmt check PASS；standard purity final42、authority79/pending25
  PASS。默认git diff check PASS；C2静态验证仍缺完整case arm，未宣称Task6完成。
- Rust使用已安装stable1.99；指定1.97.1、MSRV1.95、Clippy、all-features与最新公共API/
  compiler coverage最终验收尚未完成。

## SQLite RETURNING 名称解析一致性

- 新增 Toasty/SQLx 真实 SQLite 名称解析合同，两种驱动都复现 `MAIN.MIXED_NAME`
  正常插入却没有生成键。`task6-returning-name-resolution-red.log`：18 PASS/2 FAIL。
- 元数据表名与显式 schema 比较改用 SQLite `COLLATE NOCASE`，保持 ASCII 标识符
  大小写无关语义；继续按 temp、main、附加数据库顺序选择目标。
- `task6-returning-name-resolution-green.log`：20/20 PASS。同一合同验证混合大小写、
  临时表遮蔽主表、显式附加库大小写与多个附加库先后顺序。不同 schema 故意使用
  不同主键名，并遮蔽三个 rowid 名称，确保错误目标的元数据不会凑巧通过。
  生成键与原 RETURNING 行精确对应，私有投影不泄露到用户游标。
- `task6-returning-name-wrapper-final.log`：wrapper完整默认串行回归exit0，42结果块，
  293 PASS/0 FAIL；受影响Druid pooled Statement、生成键请求、Filter/Stat专项亦exit0，
  日志 `task6-returning-name-pooled-final.log`。
- 两个修改Rust文件rustfmt check、default diff、standard purity final42与authority79/
  pending25 PASS。本轮没有重新运行全workspace或全feature；此前workspace离线依赖
  缺失不作为通过证据。Task6未关闭，完整按列生成键/BASE/C1/C2及后续目标保持不变。

## 无 RETURNING 的 prepared 当前行生成键

- Toasty/SQLx 真实 SQLite prepared update 均复现 upsert 更新id1却发布历史id2。
  `task6-update-current-keys-red.log`：20 PASS/2 FAIL；首次修复后22/22。
- 新增 wrapper 私有完整键执行通道，默认 adapter 沿用已有单键结果；Toasty SQLite
  与SQLx自动键请求在同一owned executor/actor锁内构造私有INSERT RETURNING投影，
  再查询真实changes()。无 RETURNING 的单条多行INSERT不再只返回最后一个键。
- Prepared update和prepared batch传递完整Vec键；batch局部暂存并仅在整批成功后发布，
  失败/取消继续丢弃旧键及部分键，保留现有无损部分计数和消费队列语义。
- 将相同合同扩展到generic execute后，真实运行还复现Toasty剩余旧通道：
  `task6-update-generic-keys-red.log`：21 PASS/1 FAIL。generic更新分支接入相同通道，
  与SQLx一起验证update/generic/batch三入口的upsert、多行、忽略插入和显式id0。
- 补充真实WITHOUT ROWID、视图INSTEAD OF触发器与BEFORE忽略触发器测试。
  首次 `task6-update-current-keys-final-green.log` 实际为22 PASS/1 FAIL，名称保留但
  不作为GREEN证据：视图的未知rowid被SQLite当成双引号字符串，私有列类型不正确。
  metadata增加目标type识别，视图没有可寻址rowid时投影NULL；不取连接历史键。
  `task6-update-current-keys-schema-green.log` 最终23/23。视图外层changes为0，
  同时真实底表已写入id70，证明计数没有由RETURNING行数伪造。
- 新增方法均属wrapper私有实现，没有更改公共RDBC更新结果类型。其他驱动默认
  仍保留原单键能力，不宣称MySQL等真实多行键已验证。
- 7个修改Rust文件rustfmt check、standard purity final42与authority79/pending25 PASS。
  Task6整体仍FIX_REQUIRED；任意列生成键、完整BASE身份/Filter/disposition、C1/C2、
  最新API/compiler/all-feature/MSRV及全Task6复审仍待完成。普通Statement/Connection
  执行入口未全部接入此次prepared完整键通道，不能声称所有入口已统一。
- 核心完整串行回归 `task6-update-keys-core-final.log`：exit0，200结果块，
  3372 PASS/0 FAIL，含1doctest、23/23生成键专项及已有批次取消验证。
- 校验器自身 `task6-update-keys-verifier-final.log`：37/37 PASS。C2 source-only
  `task6-update-keys-c2-final.log` 仍exit1，缺完整参数化case arm，未放宽门禁。
- SQLite完整键路径增加表属性/列/索引与changes查询，正确性与性能分别验收；
  性能预算尚未验证，须在后续NFR/benchmark门禁中评估。

## Prepared 列请求的 BASE SQLite 合同恢复

- 重新核对 BASE `5afe8609b177544ff7d18461aa84f69efe8ee90e` 的 Toasty
  `execute_prepared`：prepared 列索引/列名请求接受并按 SQLite 单 rowid 语义执行；
  普通 Statement 的对应列选择重载仍明确不支持。此前把所有 prepared 列请求统一拒绝
  是迁移回归。“任意多列生成键投影”不是该 BASE 合同的要求，不能用其未实现阻止
  恢复真实已有行为，也不能声称本次实现了任意列投影。
- 新增真实 pooled Toasty/SQLite 合同覆盖 id 索引、id 名称、反序多索引和多名称。
  `task6-toasty-column-request-red.log`：运行时 0 PASS/1 FAIL，失败明确为准备阶段
  `connection_prepare_statement_generated_keys`，不是编译错误。
- Toasty SQLite prepared 接受三种非 None 请求，共用已有真实 rowid 执行逻辑；
  pooled/external 请求传播和缓存身份保真保持。SQLx 仍没有列请求实现，原拒绝专项
  明确限定 SQLx；不把 Toasty 支持扩大为所有驱动支持。
- `task6-toasty-column-request-green.log`：生成键专项 24/24 PASS。新增合同验证
  实际插入计数、单列键 metadata、精确 id1–4、EOF、prepared 关闭级联关闭键游标，
  以及四种请求分别占据四个缓存条目。完整 BASE generic/Filter/statement 身份合同
  尚未全部迁入 C2 参数化分支，Task6 仍 FIX_REQUIRED。
- 最终三核心默认串行完整回归 `task6-toasty-column-core-final.log`：exit0，
  200 结果块，3373 PASS/0 FAIL，含 1 doctest 和生成键专项 24/24。
  两个修改 Rust 文件格式检查、默认 diff、standard purity42 和 authority79/pending25
  PASS。`test_verify_task6_deleted_test_parity.py` 自测 14/14 PASS；C2 source-only
  仍失败于完整 generic/generated-key 参数化分支，日志
  `task6-toasty-column-c2-final.log`。这些结果不能替代最新 API/compiler/all-feature/
  MSRV/整体验收。

## 全 workspace 网络验证的最新证据

- 按现有锁文件下载缺失的依赖后，原 anymap3 缺失阻碍已消除；没有升级依赖版本。
  `task6-update-keys-workspace-final.log` 实际进入全 workspace 测试，但 Admin gRPC
  套件 36 PASS/10 FAIL，主要为 5000ms setup/live-stream 超时及 Windows TCP 10060。
  Cargo 在失败处停止，不能声称其后所有 crate 已验收。
- `task6-workspace-grpc-isolated.log`：快照容量失败项单独执行 1/1 PASS。
- `task6-workspace-grpc-suite-recheck.log`：同一个完整 gRPC 套件串行复跑
  45 PASS/1 FAIL；剩余 reset 重连项仍未等到 live stream。
- `task6-workspace-grpc-reset-isolated.log`：该 reset 项随后单独执行 1/1 PASS。
  单项通过不能证明整套稳定，也不足以确定为环境、资源竞争或产品协议根因。
  未放宽 deadline、跳过测试或修改生产网络实现，完整 workspace 门禁保持未通过。
