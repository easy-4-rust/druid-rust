# Task6 完整动态Statement Filter合同恢复（2026-10-07）

Task6仍FIX_REQUIRED，完整18顶层任务保持开放。

## 原合同与标准映射

固定BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原result_set_filter_semantics_test.rs#pooled_result_set_routes_dynamic_statement_identity_through_filter。正式替代core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_routes_dynamic_statement_identity_through_filter。

真实Toasty SQLite、连接id84/sqlite-statement-filter、先replacement后source创建顺序、SELECT1及两个Filter保留。默认返回原逻辑创建语句；标准Statement角色存在，Prepared/Callable角色不存在且对象开放。原is_same_statement具体类型判断在唯一标准Box接口下由双向真实fetch_size变更验证：source11->returned11，returned17->source17；replacement31->returned31，returned37->replacement37且source仍17，source19不改变returned37。对象开放与独立性断言继续保留，不以两个相同静态值或Box地址证明共享身份。

失败原精确DriverError(filtered statement failure)、实际source.exception_count1、三次result_set_get_statement原有序日志及真实关闭全部保留。固定BASE调用/断言观察下限通过；只绑定这一原案例，未减少1283分母。

## 生产实现

ResultSetFilter新增默认standard_result_set_statement，只调用next并返回Option<Box<dyn标准Statement>>。FilterChain私有递归依注册顺序包围，末端来自原logical_owner.statement_handle或普通Statement共享clone，不访问raw cursor、不新增第二驱动端口。get_statement先拒绝失效租约，Filter结果经过既有policy分类并仅记录原创建语句一次。有效租约中已关闭游标/语句的同一关闭身份仍可查询。公开默认方法按规格§13追踪；API/compiler/SemVer/external-consumer完整门禁仍OPEN。

## 实际验证

- 初始接口RED exit101：standard_result_set_statement尚不存在，1编译错误。日志task6-complete-statement-filter-interface-red-2026-10-07.log。
- 默认hook存在但未接入实际生产路由：88731 exit101/117PASS1FAIL，replacement期望31实际source17。日志task6-complete-statement-filter-behavior-red-2026-10-07.log。
- 路由修复89337 exit0/118PASS0FAIL。日志task6-complete-statement-filter-routing-green-2026-10-07.log。
- 两组补充控制72103 exit0/120PASS0FAIL：A/B默认正向进入反向成功退出、闭合后的同一关闭身份、旧租约无hook、B空句柄短路不进入C、B精确错误A/B反向error且实际异常1、无创建语句返回None。日志task6-complete-statement-filter-controls-green-2026-10-07.log。
- 正式绑定后27093 exit0/五结果块145PASS0FAIL：lib120、FilterStat21、矩阵1、外部Statement1、外部读取Filter2。外部实现只使用公开标准接口，真实DruidPool验证共享身份/顺序/空短路/精确错误/租约提前拒绝。上述120/118是重复范围，不相加。日志task6-complete-statement-filter-binding-regression-2026-10-07.log及summary.json。

以上全部使用默认Rust1.97.1 locked/offline串行，不是完整workspace/MSRV1.95/all-features结果。

## 正式台账与剩余范围

1127EXACT/156PARAMETERIZED；1283原七字段逐项固定。仅这一旧参数化case退役，反向调用索引与派生文档同步。新鲜全量源码观察1264PASS19FAIL，不作为行为完成率。验证器17自测、final purity48、authority79/pending25/physical73固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3及派生文档79、九Rust文件格式/diff通过。

完整C2 4429终态exit1，首个缺口推进至pooled_result_set_routes_metadata_platform_handle_through_filter。日志task6-complete-statement-filter-c2-2026-10-07.log，整体C2仍FAIL。其余Filter合同、Task7八项义务、Task8/P05–P07/API/SemVer/MSRV/features/全仓新鲜回归/完整审查均OPEN。未安装/切换分支/提交/推送/发布。


## 所有者受影响回归终态

68362 exit0，integer owner3 +metadata ownership3 +buffered matrix7 +Callable matrix9，共22通过、0失败（默认1.97.1）。实际Callable叶角色、原句柄Drop后共享关闭、可变unwrap同一所有者、旧generation Metadata/result关闭及完整Callable动态所有者均通过。日志task6-complete-statement-filter-owner-regression-2026-10-07.log及summary.json；与145绑定回归互不重叠，合计167通过、0失败。局部回归不是全仓发布证明。全部本轮Cargo/C2句柄终态；下一完整Metadata Filter，19源码合同缺口及完整18顶层范围不变。
