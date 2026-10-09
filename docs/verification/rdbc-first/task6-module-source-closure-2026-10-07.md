# Task6 源码闭合门禁修复与当前剩余证据（2026-10-07）

本轮修复测试模块声明与旧身份注释，没有修改生产行为、原案例断言、固定迁移分母或检查器规则。完整18个顶层任务保持开放，Task6仍为FIX_REQUIRED。

## 实际修改和运行

- connection_defaults.rs 移除五个与Rust默认模块路径完全相同的path属性；保留cfg(test)及五个模块声明、文件、方法和全部断言。
- Wrapper私有租约夹具的说明改为“外部池租约的标准委托合同”，消除旧标准身份名称；类型与委托行为不变。
- 修改前两个门禁实际exit1，分别拒绝上述path属性和夹具注释中的旧身份。
- 修改后执行两个包完整lib测试，session80425终态exit0：Druid105、Wrapper10，共115PASS/0FAIL/0ignored。
- 日志：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-module-source-closure-regression-2026-10-07.log。
- 此次使用stable工具链与locked/offline；不是MSRV、all-features或完整workspace验收。

## 门禁仍未闭合

- verify_no_physical_standard.py当前exit1：下一个path属性在druid_pooled_connection.rs。只读清点还存在25处：5处与默认模块路径等价，20处涉及嵌套夹具路径；不可直接移除后者使测试脱离编译。
- verify_druid_single_standard.py当前exit1：prepare_statement的pre-raw begin_admission顺序。CodeGraph确认该方法委托prepare_statement_with_generated_keys；必须验证委托路径的真实登记顺序，不以放宽检查或重复登记替代。
- verify_wrapper_standard_impls.py仍exit1：prepared参数跟踪调用点基线漂移，未在本轮修复。
- 本次两处修改没有使整个源码门禁通过；不得用历史结果覆盖这些新鲜失败。

## 范围与后续

固定1283合同账本和1122EXACT/161PARAMETERIZED绑定未改。最近完整源码观测审计1259PASS/24FAIL仍为历史基线，24项ResultSet Filter尚待完整恢复。Task7八类义务、Task8及P05–P07、公共API/SemVer/MSRV/features、完整workspace新鲜回归和最终审查保持开放。

下一步完成实际模块路径闭合，核对资源登记与跟踪证据，继续完整Filter24迁移。不把本轮115个库测试通过视为计划完成。
