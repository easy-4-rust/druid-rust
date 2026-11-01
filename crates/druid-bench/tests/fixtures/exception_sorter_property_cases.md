# Oracle 自定义错误码配置边界语料

`exception_sorter_property_cases.tsv` 是独立补充资产，共 118 条；不是原始 Java 测试文件的替代品，也不修改 `exception_sorters.tsv` 的 576 条冻结输入。

- 来源：当前 `AbstractOracleExceptionSorter#configFromProperties` 的 `Integer.parseInt` 行为；JDK 17.0.12 实跑 `Character.digit(char,10)` 得出 37 段 BMP 十进制数字。
- 分母：每个 Oracle / OceanBaseOracle 分类器 37 段数字 + 22 个边界输入，共 2 × 59。
- 边界：i32 MAX/MIN/溢出、空白、正负号、全角符号、代理项数学数字、上标/分数、混合数字、前导零、空项、重复项、无效后有效项。
- 八字段与现有 exporter 一致：case_id、sorter、error_code、SQLState、message、recoverable、cause、properties。`-` 保留原语料可空标记；尾列允许空字符串。
- 只通过两侧生产分类入口观测 true/false。集合追加/替换/重复及不扩大默认错误码由 `crates/druid/tests/exception_sorter_property_contract_test.rs` 补精确断言。
- RED：84 MISMATCH；GREEN：118 MATCH。没有增加 approved difference。
- 生成器、JDK 探针、源码/资产哈希及原始结果：[逐方法核验报告](../../../../docs/verification/method-semantics-20261101/README.md)。生成器用于复核派生资产，不得借机改写原始 576 条输入或 source hash 清单。
