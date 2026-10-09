# Task6 剩余33项 Clippy 诊断修复（2026-10-08）

正式事实源为 P04 single-standard unification。前次全范围 Clippy session69542 实际 exit101，剩Druid lib test24项与Wrapper lib test9项。本轮不新增规格、不放宽门禁、不移除测试。

## 修复范围

八个测试或辅助文件中的33项诊断已按等价语义修复：

- ConnectionDefaults 原holdability预期1/0改为i32::from(bool)，setter失败的停止顺序与原期望不变。
- 原底层Connection、ResultSet及外部租约Statement/Prepared/Callable对象地址转换使用std::ptr::from_ref；保留cast::<()>()与原usize身份比较，具体对象ptr::eq和接口addr_eq仍独立存在。
- StrongGetterProbe仅将两处Option<i32>的clone改为解引用，Calendar等非Copy参数继续clone。
- NavigationDefaultsChainHarness十二处去除Ok(...?)，保留原into_boolean/integer/string及DruidError::from转换和真实Filter调用。
- 外部租约三个原地址相等观察改用assert_eq!，地址、对象移动、LOB身份、委托调用及关闭断言保持。

无新增allow或改变Cargo feature范围、原479方法/1283测试身份、超时与断言门禁。

## 输入和运行

格式化后cargo fmt --all -- --check实际exit0。remaining33-inputs.json在运行命令前捕获，沿用1092源码和58控制输入全部key；相对前轮只有上述八个Rust文件变化，控制输入无变化。

两个crate的完整all-features lib测试原session15350以及完整C1源码验证原session8604正在运行，后者保留完整C2子检查且使用--no-execute。源码验证即使通过也不是runtime验收。

## 尚未完成

本报告首次写入时测试和源码验证未取得终态。后续须取得严格全workspace/all-targets/all-features Clippy的实际结果。Task6 FIX_REQUIRED、Task7八项义务和全部18顶层任务保持OPEN。

2026-10-08 剩余33项修复终态：Druid/Wrapper完整all-features lib原15350 exit0，150+16=166PASS/0FAIL；C1 source原8604 exit0，479映射/442选择器，executed=no且完整C2子检查保留。原正式Clippy session72889实际exit0/57.55s，workspace/all-targets/all-features/-Dwarnings通过，累计53诊断全部消除。1092源码+58控制输入终态无变化，七证据文件逐字节镜像。fmt check exit0，两个lib linker stdout警告仍保存。Task6其余compiler/API/SemVer/MSRV/features/network/review门禁未闭合，Task7八义务/18顶层OPEN。见task6-remaining33-lint-recovery-2026-10-08.md。

2026-10-08 新鲜完整C1实际运行已启动原session20187，继承remaining33-inputs.json的1092源码+58控制冻结，默认执行并保留完整C2子验收；没有no-execute或分母缩减。尚无终态，不声明runtime通过。上一轮fmt、166/0及严格全范围Clippy已终态PASS，Task6 FIX_REQUIRED/18 OPEN。
