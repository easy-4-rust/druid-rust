# 设计与冻结批次

本批次是 core 的 Filter 类身份缺陷修复，不是完整 FilterManager 或 core 对象验收。
源：FilterManager.java#existsFilter；目标：filter_chain_impl/chain_a.rs#contains_filter_class_name。
Java 基线 fa8dc9912637a2f729eef9f55356621fec18d40e；Rust 基线 b7c2afc59cf67e11f2d66fe7769093ca2d7f93f9（保留既存脏文件）。

使用精确字符串相等比较，不引入依赖，不改变工厂注册和查找策略。回归分别验证正常分支与 UTF-16 长度触发的直接加载分支；断言工厂调用次数、三条 filter 链长度和有序类名。
测试保持 V2_MIRRORED 标签，没有 Java oracle 参与不得称为差分。全局源用例、资源及对象台账不晋升。

回退边界为本批次两个 Rust 文件的精确差异，禁止重置工作区。
