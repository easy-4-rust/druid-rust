# 完整连接状态合同准备（2026-10-06）

当前完整 workspace 同一62698/cargo74560仍通过工具handle与实际进程确认LIVE；曾核实子rustc从Wrapper metadata actor集成目标继续推进。没有重启、终止或修改Rust/Cargo输入。本轮没有完整workspace终态，旧3733PASS17FAIL属于修复前源码。

完整原案例SHA `0c7b4fe299716a981abf140005d6347f6e50299f54e23334f9e991557d826c71`，8assert/6assert_eq。原0->1->0 holdability、2->8->2->8 isolation、keep选项及真实lease Drop1均保留。映射草稿改用既有rdbc_code，避免重复增加jdbc_code。由未改动的正式验证器对提取的完整BASE body重新核对SHA与源码观察floor通过。首次把body-only文本直接交给完整函数解析器产生StopIteration，补回原签名后验证通过；不是产品失败，也未改变验证逻辑。

CodeGraph与固定BASE核对：capture_standard硬编码holdability0/isolation2；needs-reset/reset缺两项，holder.reset忽略keep参数，logical recycle调用reset(false)。原reset相对顺序是read_only -> holdability -> isolation（若不保留）-> auto_commit，catalog/schema现有政策还需保持。DriverValue(i32)为拟议描述符扩展，沿既有rdbc_code保留真实0及原数字，不能把原0断言替换成2。PreparedStatementKey::standard当前unknown分支统一0，必须以真实数字保留缓存身份并实测。若隔离快照改用标准signed类型，公开u8->i32影响须纳入API/SemVer审计，尚未声称批准或完成该API变更。

非Cargo文本草稿包含完整原案例、状态对象、真实lease Drop、精确getter/setter错误探针及7类补充控制：holdability数值/默认保持；缓存键区分；原复位顺序与schema/keep政策；两种unsupported各自跳过；driver/closed错误精确传播；setter失败停止；signed隔离数值不窄化。rustfmt语法检查通过，全部NOT_COMPILED/NOT_RUN/NOT_BOUND。无生产实现、测试目标接入或正式迁移绑定。准备manifest保存格式化后的各文本SHA。

后续12个Recycle缺口的完整固定BASE源码及每个body也已提取，原SHA/断言/调用记录写入recycle_complete_preparation_manifest.json；仅原文准备，未映射/运行/绑定，不增加完成数。

正式源码观察仍1245PASS38FAIL，1108exact175parameterized；完整C2仍首败connection_defaults_capture_compare_and_reset_complete_state。完整18顶层任务、Task6FIX_REQUIRED、Task7八义务、全C1/C2/API/SemVer/MSRV/features/真实数据库/生产化/最终审查继续开放。下一步同一全仓终态后先接入完整测试获得真实RED，再修复标准状态捕获/复位及其调用传播；不能把准备当完成。
