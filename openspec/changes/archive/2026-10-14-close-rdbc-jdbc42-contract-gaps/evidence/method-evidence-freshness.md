# 8.3：方法证据新鲜度与状态区分

## 新鲜度约束

门禁现要求台账首行绑定 Java 清单 SHA-256、schema 版本和 OpenJDK source commit。每个已关联方法的 Rust 源文件及正/负向测试源文件都要记录 SHA-256；路径越出仓库、文件缺失、函数名失效或哈希变化都会让门禁失败。该绑定用于发现证据陈旧，不会自动重新运行测试，也不会因手工更新哈希就把能力升级为通过。

门禁输出区分分母、已链接、未评估、未运行和失败层数量。层状态 `PASS`、`FAIL`、`NOT_RUN`、`NOT_APPLICABLE`、`UNASSESSED` 与能力状态 `SUPPORTED`、`PARTIAL`、`UNSUPPORTED`、`NOT_RUN`、`UNASSESSED` 分开表示。`NOT_RUN` 缺少原因会失败；对 `SUPPORTED` 和 `UNSUPPORTED` 的证据及精确错误要求沿用 8.2 的 fail-closed 规则。

## 负向验证

`test_method_evidence_gate.py` 包含故意过期的测试源码哈希、Rust 源码哈希和 Java 清单版本三类负向用例，均确认门禁拒绝旧记录；另有 `NOT_RUN` 缺少原因、正向证据缺失、负向证据缺失、只有 SQLState 无精确错误四项拒绝用例，以及完整正向状态与显式 `UNASSESSED` 分母通过控制。运行结果为 10 passed。

## 验证结果

| 命令 | 结果 |
|---|---|
| `python -m unittest discover -s tools/java8-api-inventory -p 'test_*.py' -v` | 10 passed |
| `python scripts/verify_java8_method_evidence.py` | `EVIDENCE_GATE_PASS inventory=2001 ledger=2001 linked=9 unassessed=1992 not_run=0 failed_layers=0` |
| `python -m py_compile scripts/verify_java8_method_evidence.py tools/java8-api-inventory/test_method_evidence_gate.py` | 退出码 0 |
| `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` | valid |
| `git diff --check` | 退出码 0 |

已将字段语义和证据刷新流程写入 [工具说明](../../../../tools/java8-api-inventory/README.md)。本次只保存文本台账，不保存编译输出、JDK 归档或大型二进制。

## 文件 SHA-256

```text
scripts/verify_java8_method_evidence.py 0DE8949C9203CB753396DD75785FDFA266DD37AB367C5A74ED0876F9D3A2FA17
tools/java8-api-inventory/test_method_evidence_gate.py BEE3869F326F45BDEF1E60A4B577EEB6FDB54847782526CA381D5631BC103CB2
tools/java8-api-inventory/README.md 53DD89066610226D18D7AC35F87BB50C28E6B4645549B2092BBA6EC1590E0919
docs/verification/rdbc-first/java8-method-evidence.tsv 7AEA87C4ABA8181CFCF4DB061812B45892E6E097731AC31BDA175FDD313B7B8B
```
