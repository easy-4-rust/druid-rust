# Task6：标准字符串读取及 UTF-16 统计恢复（2026-10-05）

## 固定来源与实际 RED

规格：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`；计划为对应 single-standard-unification。
固定 BASE：`5afe8609b177544ff7d18461aa84f69efe8ee90e`；原案例 `druid_pooled_result_set_differential_test.rs#read_string_length_increments_on_string_get`。

完整保留真实 SQLite 五行表、SELECT name FROM t LIMIT 1、初始长度0、next、string(1)、长度大于0和关闭。新测试直接观察生产 Filter 上下文，无独立测试计数器。

修复前 `task6-string-read-red.log` 退出101，诊断12PASS/2FAIL：原始长度仍0；Unicode动态读取预期3实际0。属于实际运行时失败。

## 生产路径

唯一 ResultSet 契约增加默认 `string_value` getter，返回String/NULL，默认复用标准值读取并拒绝其他类型，允许驱动覆盖原生字符串转换。它不包含 Druid owner/state/stat hooks。

ResultSetExt::string 委托这个可动态分派入口；DruidPooledResultSet 覆盖它，通过同一资源锁、列映射、Filter 和 policy 错误分类，在成功得到 String 后累计 Java UTF-16 长度。标准 around-chain 复用现有 value hooks，仅末端选择字符串 getter；不存在第二套 raw/logical trait tree。

通用 value/object 不累计字符串统计；NULL、类型错误、缺失列标签及关闭后的失败不增加计数。UTF-16 测试真实执行 SELECT 'A😀' AS text_value, NULL AS absent, 7 AS number，分别覆盖无Filter和FilterAdapter路径，以及Box dyn ResultSet共享游标：两次读取长度3→6，后续失败/NULL仍6。

## GREEN 与回归

日志均在 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

- `task6-string-read-green.log`：14/14 私有诊断测试PASS，退出0。
- `task6-string-read-regression.log`：Druid lib73、FilterStat21、结果集矩阵5、Metadata ownership3、Wrapper lib9、标签2、当前结果3，共116/116PASS，退出0。
- `task6-string-read-rdbc.log`：15结果块，468PASS/0FAIL/0ignored，含1doctest，退出0。其中包含已有生成435测试；不宣称C1的479方法生产行为已认证。
- verifier自测15/15PASS；final purity检查43文件；authority79行、pending25、physical73，固定SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`；格式与diff检查通过。
- `task6-string-read-c2.log`：退出1，下一处 result_set_constants_match_java 缺少明确分支；整体C2仍FAIL。

一个BASE映射改为明确的私有lib入口；原1,283分母和摘要不变，目录1,077exact/206parameterized。结构性未明确分支67：buffered29、lease2、recycle12、Filter24；已有明确分支不等于完整行为认证。

## 全量与剩余门禁

前一版 Metadata 全量 `task6-metadata-proxy-workspace.log` 已终态退出101，240结果块，3677PASS/2FAIL/0ignored，含2doctests；仅Callable helper/matrix original prepared5vs6。此结果早于本次字符串修改，不能替代本次全量验收。

本次已执行相关116项回归与完整RDBC包468项；没有重新执行字符串改动后的全Workspace。新增默认公共方法需最终API快照/SemVer验证；当前cargo-public-api缺失，未安装工具或使用bootstrap规避。all-features/MSRV等仍未完成。

Task6仍FIX_REQUIRED，C1/C2、Callable5vs6、owner/callform身份、API和最终审查未关闭。Task7仍8组件，Task8与P05-P07仍在完整目标内。所有本轮启动句柄已终态；没有分支/提交/推送/发布/安装或目标完成声明。
