# Task6：C2 剩余案例与字符串读取路径核对（2026-10-05）

本次只读使用现有迁移解析器枚举 207 项参数化映射，再在当前可执行测试主体中检查每个案例是否存在唯一明确的字面量 match arm。没有修改验证器、迁移分母或 Rust 源码。

| 矩阵 | 缺少明确分支数量 |
| --- | ---: |
| Buffered ResultSet | 30 |
| Standard Connection Lease | 2 |
| Recycle | 12 |
| ResultSet Filter | 24 |
| 总计 | 68 |

这仅是结构性最低工作量。已有明确分支的 139 项仍需通过原始断言、调用路径、数据和运行证据检查，不能从分支存在推断已完成。原始测试分母仍为 1,283。

下一处已确认行为缺口来自固定 BASE 5afe860 的 read_string_length_increments_on_string_get：真实 SQLite 五行表，SQL SELECT name FROM t LIMIT 1，初始读取长度零，next 后 string(1) 成功，长度大于零，关闭。

CodeGraph 与源码确认当前 ResultSetExt::string 使用通用 value；DruidPooledResultSet::value 只执行开放检查、列映射和 value_core。上下文虽然有 UTF-16 计数方法，当前标准字符串读取没有触发它。不能在所有 Value::String 读取上计数，否则 getObject/general value 也会计入。修复需要真实标准动态字符串读取入口，并保留驱动分派、Filter、错误分类，以及成功非NULL字符串才累计的时机。

当前上一轮全量回归 session18117 / task6-metadata-proxy-workspace.log 仍由句柄确认运行中，尚无终态。不重启、不终止，也不在同一编译运行中修改 Rust。上一轮79/79针对性结果不替代全量。

完整目标仍是 Task6-8 和 P05-P07；没有完成声明。
