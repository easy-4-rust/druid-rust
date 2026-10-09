# Task6 真实 SQLite 标量合同与布尔默认转换恢复（2026-10-05）

## 目标与状态

继续完整 P04 Task6–8/P05–P07；Task6保持FIX_REQUIRED。前一轮是实际进展：重载合同和 BufferedRowSet 行数实现有独立及全量证据。本轮恢复10个完整 BASE 真数据库场景，不能把剩余通用 fallback 认定为迁移完成。

来源：`5afe860:crates/druid/tests/druid_pooled_result_set_differential_test.rs`。恢复 boolean、bytes、nullable date、double、float、long、string、national string、object、was_null 各原函数的 SQL、建表/插入、5行测试数据、取值/label调用、转换/NULL断言及关闭生命周期。新增10个显式literal case arms与共享helper，再由独立测试入口运行10个场景。真实 Toasty SQLite factory -> standard holder -> Druid pooled Connection -> Statement -> ResultSet，不用 BufferedRowSet 构造值代替原驱动。

已批准规格 ADR-RDBC-006 第1节要求保留原可观察语义；本次是恢复该既有需求，没有缩减验收目标。

## 实际 RED/GREEN

`task6-buffered-sqlite-scalars-first.log` 编译通过，运行0 PASS/2 FAIL；boolean真实SQLite返回Int(1)，ResultSetExt默认boolean只接受Bool，错误 expected boolean got1。原 pooled getter委托 rdbc_result_set.rs#value_to_boolean，BASE明确支持Bool/Int/Float/Decimal/String，NULLfalse。

恢复全部 BASE 布尔转换：数值非零为true，String仅"1"或case-insensitive"true"为true，不额外trim或泛化文本；其余类型仍错误，raw value错误仍先返回。`task6-buffered-sqlite-scalars-boolean-fix.log` 2/2PASS，包含独立10场景与主矩阵。`task6-boolean-conversion-contract.log` 1/1PASS，18种原始值的index/label/was_null，以及未定位/关闭/不支持bytes边界。

## 纯净性门禁修复

首次livepurityERROR把合法 `value != 0` 误识别为 `value!` 宏。新增可通过rustc的永久回归，`task6-boolean-purity-inequality-red.log` 0/1ERROR证明该误报。macro_invocation_sites现在要求!后接Rust宏调用delimiter，再做现有绑定来源校验，既不新增allowlist，也不允许backend宏来源。

`task6-boolean-purity-selftests.log` 全105/105PASS，exit0；live final purity43PASS。此项是验证器语法识别修复，不是放宽原测试分母或接受违规宏。

首次C2静态检查还拒绝了恢复源码注释中的退休类型名称；将该注释改为中文、准确描述原label调用未对结果作断言，未修改生产/断言/验证器规则。最终`task6-buffered-sqlite-scalars-c2-final.log` --no-execute exit1，推进到 callable_statement_none_for_regular_statement 显式分支。1283分母保持；C2整体未通过，且静态检查提前停止，不证明后续所有分支。

## 当前验证边界

authority79 / pending25 / physical73 / SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc PASS；3文件rustfmt check及git diff --check exit0。

新鲜默认workspace locked/offline serial --no-fail-fast session2299 已终态，exit101 /238结果块 /3658PASS /2FAIL /0ignored，含2doctests。日志 task6-buffered-sqlite-scalars-workspace.log。仅既有Callable错误helper/总矩阵prepared5vs6失败，其余目标通过，新增10场景与布尔转换测试已包含在该全量结果中。先前3656/2属于旧源。所有本轮Cargo/Python/C2会话均终态。

## 剩余

普通Statement的Callable/Prepared负身份与ResultSet原始生命周期/元数据/列映射/统计/强类型/default/resource合同仍需完整逐项恢复；原int_by_label拒绝与现代label支持的迁移裁决也未关闭。Callable prepared5vs6/typedindex owner/cache/普通Prepared创建后拒绝/精确调用形式身份仍开放。C1/C2/API/compiler/SemVer/all-features/MSRV/最终review、Task7八组件、Task8及P05–P07均保留。没有提交、推送、发布、安装或分支操作。
