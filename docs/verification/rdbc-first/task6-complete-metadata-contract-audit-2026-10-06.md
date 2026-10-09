# Task 6 完整元数据合同编译核查

## 固定原案例

BASE `5afe8609b177544ff7d18461aa84f69efe8ee90e` 的
`crates/druid/tests/druid_pooled_result_set_semantics_test.rs#result_set_metadata_preserves_complete_rdbc_column_contract`，
body SHA-256 `2e39d689dc3ebcfc298b88d3dcb0ee2382b9e7529c36f928bc6d33ef017005c9`。

完整草案保留全部 31 个原断言宏：首列的 24 个字段观察、10 类列类型的
JDBC 数值代码/类型名/类名/符号性，以及三态可空性代码。循环中的四项断言
对全部 10 类类型执行；本轮编译失败，尚未执行任何运行时断言。

草案位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/complete_metadata_contract_standalone.rs`。
草案尚未进入正式 Cargo 测试，也没有绑定迁移账本。

## 构造与接口映射

- 标准快照使用现有 `ResultSetMetaDataSnapshot`，全部 getter 接收合法 `ColumnIndex`。
- 原 `with_origin(name,schema,table,catalog)` 映射为
  `with_driver_type(name,kind)` 与 `with_origin(catalog,schema,table)`，保留原字段值。
- 原 `with_shape(display,precision,scale)` 映射为
  `with_dimensions(precision,scale,display)`，保留 24/19/4。
- 八个原 flag 分别进入现有 `with_flags` 与 `with_writability`。
- 驱动类型名称 MONEY 和厂商类名 com.example.Money 显式保留，不修改标准构造器的中性默认值。
- 三态 Unknown 仍要求代码 2、is_nullable=false；不把 Unknown 合并为 Nullable。
- 类型枚举的四个原查询和可空性枚举的代码查询仍明确出现在完整草案中；不以测试内部映射表模拟缺失生产逻辑。

## 实际编译证据

使用当前已编译的标准包产物
`F:/codex_build_cache/druid_rust/task6_strong_read_20261006/debug/deps/libdruid_rdbc-85c04db4ca3ea2b6.rlib`，
由 stable rustc 直接编译完整 standalone 测试。
命令终态 exit 1，9 处 E0599；日志为
`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-metadata-compile-red.log`。

缺失方法是 `ResultSetColumnType::{rdbc_type,type_name,class_name,is_signed}` 与
`ColumnNullability::rdbc_code`。重复诊断来自完整首列和循环/三态观察。
这是缺失接口的编译证据，不是行为 RED，不证明当前已有 getter 运行失败。

独立核对会话 23095 终态 exit 0：对象身份、警告/元数据、完整元数据三个
草案均通过现有未放宽的 `verify_observation_floor`。此结果仅证明原观察数量
得到保留；完整元数据仍无法编译，明确说明源码观察检查不能替代运行时验收。
三个草案均未增加正式通过分母或改变迁移账本。

CodeGraph 复核当前源码确认这些方法不存在；Druid 私有元数据代理已提供
单列 rdbc_type/nullable_code/is_nullable 派生，但不能替代原枚举的完整类型身份观察。
后续恢复须遵守唯一标准合同和公开 API 门禁，并明确新增 Jdbc/Named 类型的语义；
本轮未选择或实现新的枚举 API，也未恢复第二接口树。

## 当前边界

全仓回归 session 15535 / Cargo PID 68796 仍真实运行，rustc 子进程继续变化。
本轮仅新增独立草案和证据文档，没有改变该回归的 Cargo 源码输入。
固定 1283 账本和 1232 PASS / 51 FAIL 源观察统计不变。
P04 Task 6 仍 FIX_REQUIRED，完整 18 个顶层任务仍未闭环。


## 正式恢复与准确绑定后续

两个完整案例已进入正式lib测试；1309终态exit0/lib93+FilterStat21+matrix6=120PASS/0FAIL。随后准确绑定两条原账本行，固定1283原字段不变，旧矩阵移除两个对应ID；29175绑定后矩阵6/6PASS。全1283审计40618终态1234PASS/49FAIL，登记1097exact/186parameterized。完整C2会话5925exit1推进到result_set_errors_typed_getters_and_old_lease_are_observable。详见task6-object-warning-restoration-2026-10-06.md。全仓15535此前已终态3710/2，本轮无新全仓通过声明。Task6及完整18任务仍未闭环。
