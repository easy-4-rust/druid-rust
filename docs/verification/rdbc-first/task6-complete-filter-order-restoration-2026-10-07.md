# Task6 完整 Filter 顺序、短路与错误退出合同恢复（2026-10-07）

完整原合同已执行并正式 EXACT 绑定。Task6仍FIX_REQUIRED，全部18顶层任务仍OPEN。

## 原合同保真

固定BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原body SHA0a93c5e3e304e6f82515cc14b0178c00c46aecbef9b5d05b3fd12fb51b463c09。原8个assert_eq、2个assert全部保留。

两Filter开放日志为second:open、first:open；next按first/second进入，实际原生physical:next一次，然后second/first退出。关闭按first/second进入，实际原生physical:close返回原DriverError("close failed")，错误原样返回，两个成功后置事件都不发生。独立短路链准确保留short:next-before、short:next-short，并验证原生next_count为0。

标量链保留原下标5、原生整数32、second加10、first加1，结果43与完整五条原日志一致。IntWrappingFilter通过标准Int读取描述符包围真实链，原生探针通过read_object返回Scalar(Int(32))；没有在测试主体合成最终结果或绕过生产链。

新增三个单对象测试夹具文件，原同步Physical探针与过滤器映射到现有标准异步接口。开放事件通过实际standard_result_set_open_after_with_proxy分派，next/close通过实际standard_result_set_next/close，整数通过实际standard_result_set_typed_value。没有新增或修改生产接口、第二驱动端口或过滤路由。现有实现首次运行完整原合同直接通过，不虚构生产RED。

## 实际验证

- session93094：默认Rust1.97.1完整Druid lib exit0，141PASS/0FAIL/0ignored。完整原测试实际执行，1075项冻结Rust/Cargo/proto/SQL输入终态SHA全部一致。日志task6-complete-filter-order-initial-2026-10-07.log，SHAc625d393ad684c0bd4bfd0c412af842b70938d1c54675175f6a7613c849c8367。
- session43297：保护式绑定exit0；全部1283项原始前七字段逐项不变，仅将当前合同正式EXACT并移除原矩阵占位，窄注册真实lib测试。1138EXACT/145PARAMETERIZED，验证器规则未放宽。
- session95688：绑定后完整FilterStat21、canonical ResultSet矩阵1、外部Resource Filter1，共23PASS/0FAIL/0ignored。1075项绑定后冻结输入SHA全部一致。日志task6-complete-filter-order-consumers-2026-10-07.log，SHA32b89c97fae56afbad141bb9271b33763837b0b738748074e3a349942f472b80。
- session9591：全部原源码合同审计完成，1275PASS/8FAIL；所有1283项原始前七字段保持不变。审计exit0仅表示完整审计完成，不能表述为全部合同通过。
- session11436：完整C2 --no-execute终态exit1，下一缺口为result_set_filter_defaults_delegate_and_context_defaults_are_observable。整体C2仍FAIL；源码检查不代表全部替代测试执行通过。
- 验证器自测17/17、final purity48、authority79/pending25/physical73与固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、四Rust文件格式检查和git diff --check通过。

lib141与消费者23分别报告，不将部分范围累计成全workspace通过。历史警告保留，不作零警告声明。

## 剩余范围

Task6源码合同剩余8项，其他完整门禁继续开放。Task7八义务、Task8、P05/P06/P07及全部18顶层任务未关闭。历史全workspace3795PASS/11FAIL及独立Admin49PASS/11FAIL继续保留；没有当前新源码全workspace/API/compiler/SemVer/MSRV/features/完整审查通过声明，没有系统安全策略更改。

下一完整默认委托及上下文合同原文已提取，原47eq/3assert，body SHA46a6af2e4d00d2a6d1ceec100c7f59cfcd2741ccf4414e28c290b1e49f7ba36d。原支持代码从同一BASE保存，SHAd20a88372f2ba2ab12163e702e84493f9dd0425f4fe7c124a7a3699f56c7bd96；仍NOT_COMPILED/NOT_RUN/NOT_BOUND，不计入完成。

本轮所有启动句柄均已终态。没有创建分支、stage、commit、push、安装或发布。
