# 设计

事实源为现有 rdbc-jdbc-method-conformance 规格及本变更增量。核验批次包含 ExceptionSorter 接口、九个现有厂商分类器、AbstractOracleExceptionSorter、Mock 分类及既有池处置回归；生产修复仅共享配置解析。

Java 当前 HEAD 的 11 个 oracle 来源文件逐字节匹配已有冻结清单。以 JDK 17.0.12 执行当前源码，明确 Character.digit(char, 10) 的 BMP 规则；UTF-16 代理项（包括补充平面数学数字）不被 Integer.parseInt 接受。Rust 使用同一 BMP 数字区间和负数累积的检查算术，保留 MIN_VALUE、正负号和溢出拒绝行为，不使用宽泛的 Unicode 数值属性。

新增语料独立于原始 576 条，复用同一 Java exporter 与 Rust 生产入口。先完成测试批次并运行 RED，再实施一处修复，之后统一核验，不逐对象授予完成状态。精确返回值、集合变化和逐例差分均保留；approved differences 仍单独报告。

并行文件只记录初始/结束 hash，不覆盖；Cargo.lock 的并行变化与构建快照分开记录。覆盖率报告保留全分母，分类器局部数字不能作为整仓通过证明。
