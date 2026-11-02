# Java 测试与资源初始账本

## 来源

- Java HEAD：`fa8dc9912637a2f729eef9f55356621fec18d40e`
- 来源模块：`E:/workspaces/workspace-github/druid/core`
- 工具：`rust-java-migration-testing/scripts/audit_migration_tests.py`
- 生成日期：2026-11-02

## TEST_CASE 分母

- `java-test-cases.tsv`：**4759** 个 `@Test` 方法，4759 个唯一 `case_id`，无同名冲突。
- 参数化/动态展开候选：脚本当前报告 **0**；该结果是当前解析器能力，不代表未来新增动态测试。
- 字段：`case_id`、`source_file`、`line`、`test_name`、`kind`、`signals`。
- 测试范围沿用来源审计口径，包含 `benckmark` 等目录；不按 Maven 默认 Surefire 的 `bvt` 过滤缩小分母。

## TEST_ASSET 分母

- `java-test-assets.tsv`：**822** 个资源文件，总计 **13.46 MiB**。
- 已逐字节复制到 `crates/druid-test/tests/fixtures/source-java/src/test/resources/`。
- 复制的 822 个目标文件已逐个比较 SHA-256，**822/822 一致**；明细见 `asset-copy-verification.tsv`。
- 未修改来源资源；目标只保留不可变副本。

## Parity 清单初始状态

- `docs/migration/source-test-parity.json` 已生成，包含 4759 个来源用例与 822 个资产记录。
- 当前所有来源用例的 `disposition` 均为 `MISSING`、`result_parity` 为 `NOT_RUN`；这只暴露真实缺口，不伪造映射或差分。
- 初始审计结果：来源 4759、资产 822/822 已验证；语义映射仍为 0 个有效完成用例。
- 整仓验收模块 `druid-test` 尚未达到 PASS：`status=NOT_RUN`，`not_run=4759`。

## 后续门槛

1. 按对象/方法语义建立真实目标映射，禁止只按测试名匹配。
2. 每个来源用例保留输入、断言、错误、状态、副作用和清理，并产生 golden/live `MATCH`。
3. 取消 ignored/NOT_RUN 后更新 manifest 与 runs 产物。
4. 全部来源用例完成后，才可将本账本用于完成判定。
