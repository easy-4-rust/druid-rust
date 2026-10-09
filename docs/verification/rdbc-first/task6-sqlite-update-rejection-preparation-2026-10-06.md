# Task6 SQLite 完整更新拒绝合同准备（2026-10-06）

本轮只读代码与固定BASE，完整workspace61393/cargo70220仍LIVE，未修改Rust/Cargo输入。非Cargo完整原文保存在drafts/sqlite_update_rejection_full_base.rs.txt，未接入、未运行、未绑定。

固定BASE5afe8609b177544ff7d18461aa84f69efe8ee90e的 sqlite_rejects_every_scalar_and_stream_update_without_corrupting_cursor，bodySHA 8f33bdc25cc124a51ffef0ce5a42ba463c3d5f25d5020b3ca69457598b962a1e。查询必须保留 SELECT 1 AS id, 'first' AS label UNION ALL SELECT 2, 'second'。input bytes[1,2,3]、reader文本严格 SQLite、类型映射APP.USER_TYPE->Custom(User)不得替换。

## 完整失败分母58

| 家族 | 调用数 | 原观察 |
| --- | ---: | --- |
| NULL | 2 | 下标与原标签id |
| Boolean/Byte/Short/Int/Long/Float/Double/BigDecimal/String/Bytes/Date/Time/Timestamp/NString | 28 | 14家族各2个入口；数值1/2/3/4/5.0/6.0及nullable参数保留 |
| Object及ObjectWithScaleOrLength | 4 | 下标/原标签；scaleOrLength=-1 |
| ASCII/Binary/Character | 18 | 3家族各无长度/int(-1)/long(-1)和下标/标签 |
| NCharacter | 4 | 无长度/long(-1)，下标/标签 |
| TypeMap getters | 2 | 下标Some(map)、标签None |
| 合计 | 58 | actual creatingStatement.exception_count=58 |

失败之后继续next到第二行，int(1)=2、string(2)=Some(second)，input和reader保持未关闭。不能只验证is_err，不能吞掉原负长度，不能以成功返回补齐计数。原默认更新精确UnsupportedOperation下标result_set_update_value、标签result_set_update_value_by_label；两种type-map错误也分别精确保留。

## 标准入口映射证据

CodeGraph确认当前更新采用ResultSetUpdate强类型描述符，而非57个独立生产方法。标量映射各同名variant；ObjectWithScaleOrLength原样保存-1；流映射AsciiStream/BinaryStream/CharacterStream/NCharacterStream，携带调用者克隆句柄与RdbcStreamLength/RdbcCharacterLength的Unspecified/Int(-1)/Long(-1)。测试适配只能映射到唯一update_value/update_value_by_label，不创建第二生产端口，不提前读流。

当前canonical update_value已返回UnsupportedOperation(result_set_update_value)，LOB特殊家族有独立拒绝名；当前generic update_value_by_label先find_column再调用index，因此普通标签的错误名/优先级不等于原默认。此为源差异证据，尚非该完整案例runtime RED。下一正式TDD须保留原58与完整游标/资源观察，并用实际Toasty入口确认真实行为，不再凭SQLite名称推断SQLx调用链。

若恢复generic label默认拒绝，需同时核对明确支持可更新标签的具体Buffered实现，保留其真实行为与关闭/列名验证，不能机械改变具体能力。此准备不减少当前42源码缺口或完整18顶层任务，不关闭Task6。


完整映射草稿已生成于 drafts/sqlite_update_rejection_mapped.rs.txt（非 Cargo 输入）。确认现有 task6_sqlite_diagnostic_statement_with_filters 真实创建 Toasty SQLite、DruidPooledStatement 与 StandardResourceRegistry，能够观察实际 exception_count，无需公开私有统计或测试伪计数。宏完整展开分母为2+14*2+4+2*6+6+4+2=58；保持原SQL、数字、NULL object、负int/long长度、type map、第二行和调用者资源未关闭观察。新增关闭只位于原观察之后。映射通过唯一 update_value/update_value_by_label 和类型映射getter，不建立第二生产接口。草稿尚未编译、未运行RED、未接入正式目标、未绑定账本；宏分母审阅不冒充runtime/sourcefloor证据。

本轮继续确认同一完整回归61393与cargo70220实际LIVE，子rustc34800，未改Rust/Cargo输入。当前42源码缺口与18顶层任务不减；完整C2只读复核仍exit1，下一案例保持sqlite_rejects_every_scalar_and_stream_update_without_corrupting_cursor。


草稿独立源码审阅完成：从固定git BASE重新提取原函数并核对body SHA，通过现有verify_observation_floor，原/草稿均assert5、assert_eq3、matches1，调用密度未缩减；逐项检查原SQL、UTF-8 reader、APP.USER_TYPE/User、58实际统计、第二行second、流/reader未关闭，以及Unspecified/Int(-1)/Long(-1)和两个scale_or_length=-1。审阅数据保存drafts/sqlite_update_rejection_draft_source_review.json，明确标注DRAFT_ONLY_NOT_COMPILED_NOT_RUNTIME_NOT_LEDGER_BOUND。此检查不是1283正式floor的新通过项，当前1241/42不变。

CodeGraph补充确认：当前BufferedRowSet已显式实现update_value_by_label，先find_column再实际update_value，保持缓冲行真实可更新能力；无需通过改变其路径来恢复只读standard默认。实际Toasty使用的StandardAdapterResultSet没有update_value/by_label覆盖，因此原完整SQLite的label默认错误与canonical泛型先查列再index确有源差异。下一步仍先完整案例runtime RED，再最小修正；源差异不冒充运行失败证据。

同一61393/cargo70220仍LIVE，子rustc先36640后75816，当前日志仍为编译警告，未取得终态。未编辑Rust/Cargo输入、未重启任务。
