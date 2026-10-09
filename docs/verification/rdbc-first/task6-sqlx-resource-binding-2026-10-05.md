# Task 6 SQLx SQLite 资源绑定合同恢复（2026-10-05）

状态：专项验证通过，Task 6 仍为 FIX_REQUIRED。

## 原始合同与裁决

BASE `5afe860:crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs#sqlx_prepared_resources_execute_and_batch_against_real_sqlite` 要求同步 stream/Reader 在 setter 消费指定长度；短流在 setter 返回 DriverError，负长度返回 InvalidArgument，调用者读取剩余数据后执行仍使用绑定值。此前 SQLx 全部延迟读取的假设不符合该 BASE。仅恢复 SQLite，同步物化后底层保存 RustValue；池化 Filter 保留富描述符。其他数据库的读取时机未经此次验证。

完整 canonical 用例保留短流/负 Reader、Binary/Character/NCharacter、URL/RowId、结果元数据及值、generic query、两组批次与三行数据库内容、Filter 三次执行与一次批次的描述符检查。所有八个 SQLx BASE case 均有显式迁移分支。

Ruling: 既有共享资源测试对 SQLx SQLite 的零读取预期与 BASE 冲突，改为两个 SQLite adapter 都在同步 setter 读取。保留直接 rich batch 执行阶段读取、取消前未读批次可重试、部分失败消费一次、清参清批次不新增读取、关闭后拒绝读取。该裁决不推广到其他 backend。

## 验证证据

- 恢复完整测试后的真实运行 RED：5 PASS / 2 FAIL，短流 setter 未报错；日志 `task6-sqlx-resource-red.log`。
- 同步绑定实现后，旧共享测试 RED：2 PASS / 1 FAIL，绑定读取次数实际 1、旧预期 0；日志 `task6-sqlx-resource-green-attempt.log`。不是完整修复通过记录。
- 对齐 BASE 后同一次命令终态 exit 0：SQLx canonical 7/7、共享资源执行 3/3，共 10 PASS / 0 FAIL；日志 `task6-sqlx-resource-final.log`。
- 命令：`cargo test -p druid-wrapper --test task6_sqlx_canonical_matrix_test --test prepared_resource_execution_test --locked --offline -- --test-threads=1`。
- 最新完整 workspace 回归终态 session 7819 / exit 0：235 个结果块，3646 PASS / 0 FAIL / 0 ignored，含 2 doctests，日志 `task6-sqlx-resource-workspace.log`。涵盖最终 SQLx 资源绑定实现和十项专项；其后的 Toasty 测试迁移属于新测试源码，只有独立专项通过，不混称已获新全量结果。
- C2 no-execute 终态 exit 1，首个阻断推进 Toasty `auto_commit_default_is_true` 显式迁移分支，日志 `task6-sqlx-resource-c2.log`。这只证明静态检查顺序推进，不证明完整 C2 运行通过。
- 三个相关 Rust 文件格式检查、git diff --check、标准纯净性（43 文件）和权威清单（79 行，Task7 待办 25 行）本轮通过。

## 未闭合范围

C1 生产证据、完整 C2、API/compiler/SemVer/all-features/MSRV/全阶段审查仍未闭合；Task 7 的八组件、Task 8、P05–P07 保持完整范围。最近历史 workspace 3641/0 不覆盖当前 SQLx 生产变更。不提交、不推送、不发布、不安装工具。
