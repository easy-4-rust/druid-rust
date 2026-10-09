# Task6 Prepared、Callable 与 Clippy 诊断收集（2026-10-08）

正式事实源仍为 P04 single-standard unification；Task6 FIX_REQUIRED，全部18项顶层任务保持OPEN。本报告记录真实命令范围，不扩大全仓通过结论。

## 已取得的终态

- Prepared recorder 的五字段事件改为类型别名；cleanup 去除冗余借用；空集合检查使用 is_empty。原错误、参数与集合断言保留。完整 Prepared 测试目标实际 exit0，15 PASS/0 FAIL。
- 随后的正式 workspace/all-targets/all-features Clippy 实际 exit101，Prepared 三项诊断已消除，停在 Callable 目标。
- 同范围加 --keep-going 的诊断收集原 session92273 实际 exit101：Callable测试12项、Druid lib test32项、Wrapper lib test9项，共53项。此选项只收集独立目标错误；最终仍须执行原正式命令。日志SHA256：8335238dc837434a2d080ecdc433628f69ce749bfa2476a767daf225c6b57b03。
- Callable 四个冗余 continue 位于循环末尾，删除后执行case记录与digest不变；八处指针转换改为 from_ref，保留 addr_eq 的原对象地址断言。
- metadata 八项指针转换诊断改为 from_ref().cast::<()>，保留具体对象 ptr::eq、标准角色、类型擦除入口、数据地址与调用记录断言，避免比较不同虚表。
- 原 session89781 实际 exit0：Druid完整lib150 PASS/0 FAIL，完整Callable目标9 PASS/0 FAIL，共159/0。cargo fmt --all -- --check实际 exit0。

## 输入与证据范围

callable-metadata-inputs.json记录编译期间快照，不称为命令前冻结。原1092源码与58控制输入全部保留；相对Prepared快照只有本轮两个Rust文件变化，控制输入无变化，测试终态逐项复核无变化。

Prepared、首轮诊断收集、本轮159/0日志及摘要共10个文件，从F盘原件逐字节镜像至 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-*`，SHA校验通过。

## 未完成

新的全workspace/all-targets/all-features严格Clippy诊断收集已启动原session69542，本文写入时仍未取得终态。原53项中其余33项尚待修复，后续终态可能揭示新诊断，不把20项源码修复当作已通过完整Clippy。

本轮未执行完整C1/C2新鲜验收；既有479映射/1283身份验收分母、原测试断言和门禁不变。编译覆盖再生成、API/SemVer/MSRV、feature矩阵、网络回归、Task7八项义务与其余计划均未关闭。

2026-10-08 Clippy复核终态：原session69542实际exit101，诊断由53降为33（Druid lib24、Wrapper lib9）；Callable12与metadata8已消除。1092源码+58控制输入无变化；日志与summary逐字节镜像。本轮所有Cargo句柄终态，无LIVE。Task6 FIX_REQUIRED/18 OPEN。
