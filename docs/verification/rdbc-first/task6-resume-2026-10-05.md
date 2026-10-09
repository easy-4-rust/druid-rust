# P04 Task 6 恢复执行记录（2026-10-05）

状态：IN_PROGRESS / FIX_REQUIRED，尚未关闭 Task 6，也未启动 Task 7。

## 执行基线与约束

- 基线：`f507870`，启动时工作区干净。
- 规格事实源：ADR-RDBC-006；沿用单一标准接口实施计划。
- 按项目约束保持当前分支，不创建 worktree，不推送、不发布。
- 使用 CodeGraph 定位验证器、租约回收和关闭 worker。
- 当前指定的 Rust 1.97.1 工具链目录缺少 rustc/rustdoc；此次运行使用已安装
  的 stable Rust 1.99.0 / Cargo 1.99.0，不将其视为 1.95 MSRV 或 1.97.1 门禁。

## 本次发现与修改

1. Windows `core.autocrlf=true` 将台账和 API 快照改为 CRLF，使规范字节门禁失败。
   新增 `.gitattributes` 固定相关产物 LF，并恢复工作树中的规范 LF 字节；不放宽校验器。
2. deleted-test 校验器按系统 GBK 解码 Git 的 UTF-8 源码并崩溃。
   新增真实 Git 中文源码回归测试，观察失败后显式使用 UTF-8；测试台账 fixture 显式写 LF。
3. native pool 在 fatal 同步计数后、holder 转交前关闭 worker，导致底层连接不关闭。
   新回归测试观察到 `driver_close_count=0`（应为 1）。增加内部待转交丢弃计数，
   使 worker 在真正关闭命令入队前保留；显式 close 与 Drop 两条路径均验证。

## 本轮验证证据

| 命令/范围 | 结果 |
| --- | --- |
| `python -m unittest discover -s scripts -p test_verify_task6*.py` | 37/37 PASS |
| `python scripts/verify_task6_migration_authority.py` | PASS，79 身份，25 TASK7_PENDING；SHA-256 `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc` |
| `cargo test -p druid --test task6_owner_disposition_control_test --offline` | RED 1 PASS/1 FAIL → GREEN 2/2；Rust 1.99.0 |
| 三个修改 Rust 文件的 rustfmt check | PASS |
| `git diff --check` | PASS |
| `cargo test -p druid --offline` | exit 101；已运行汇总 2032 PASS / 2 FAIL，失败发生在 external-consumer fixture 子 Cargo 更新网络索引；后续测试未执行 |
| `CARGO_NET_OFFLINE=true cargo test -p druid --offline` | exit 0；145 个测试结果汇总，2611 PASS / 0 FAIL；含 external-consumer fixture，doctests 0；Rust 1.99.0 |

## 仍须关闭的复审与验收项

- 独立只读复审：FIX_REQUIRED。除上述 native shutdown 问题外，方法表中
  435/479 selector 指向 generated strict override 测试；其预设错误断言不能证明
  canonical 默认逻辑或 adapter 行为保持。须补真实生产路径证据，不能以自测通过替代。
- method/deleted-test 静态门禁在当前 checkout 仍失败：
  `prepared_bound_setters_execute_and_batch_against_real_sqlite` 的 BASE `matches` 断言
  下限为 1，当前目标为 0。须追溯 BASE 实际语义并恢复，不得填充无意义断言。
- 旧报告 `56 CLOSED / 23 PENDING` 已过时，当前 authority 为 `54 / 25`。
- Task 6 全量方法/删除测试执行、公共 API/config/wire、所有 feature 和质量门禁未完成。
- 本次尚未执行 Task 7–8、P05–P07；全部仍属于总目标范围。
