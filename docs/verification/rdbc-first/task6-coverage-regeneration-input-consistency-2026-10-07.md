# Task6 覆盖再生成预检与输入一致性修复

完整C2沿用session13316。核查时Python13276、wrapper all-features lib Cargo70556、DuckDB build-script40160及不断推进的MSVC编译子进程均真实存活；没有重新启动C2或第二组Cargo构建。

## 实际源码预检

只读调用既有canonical_requirements和validate_canonical_source_requirements，七family/105来源义务全部通过；当前与旧清单的kind/path/function/anchor绑定差异为0，源码hash过期105行。证据task6-coverage-regeneration-preflight-2026-10-07.log/json。结果明确COMPILER_EXECUTED=no，不替代真实编译运行或105精确compiler identities。

## 再生成输入一致性

原build_manifest先跑七family，再从当前磁盘读取source hash。family之间构建输入变化时，可混合旧报告与新hash。新增控制在第一family返回时修改源码、删除来源、新增来源或修改Cargo.lock；原实现5项自测终态3FAIL/1ERROR，其中三个变化未拒绝，删除来源在稍后读hash时才FileNotFoundError。日志task6-coverage-input-consistency-red-2026-10-07.log。

修复在预检前冻结Rust/proto/SQL、Cargo manifests/lock、rust-toolchain和仓库Cargo配置；排除.git/.codegraph/.superpowers/target/node_modules。每个family之前和之后、文档构造完成及main最终验证后写入之前比较完整集合与SHA。source_sha256直接取冻结来源；任意新增/删除/修改失败。原七family/15身份、精确compiler名称、positive region、fresh report/profile规则保持。

原5项修复后PASS；加入一致输入完整七family/105行正向控制与最终验证期间变化保留原清单控制后，完整该生成器自测7项PASS。日志task6-coverage-input-consistency-green-2026-10-07.log与task6-coverage-input-consistency-final-2026-10-07.log。这些是验证器回归，不是实际编译覆盖认证。

新构建快照1058项；C2原1090清单额外含nested target中32个生成缓存.rs。最初使用新快照比较旧清单将32项列为缺失，只是排除范围差异，不是文件变化。随后按原1090清单逐文件直接读取核验，ACTUAL_CHANGED_INPUTS=[]。未删除缓存或改变原运行分母。

## 工具及未完成范围

实际cargo llvm-cov --version返回no such command；1.97.1 installed components不含llvm-tools-preview。核对官方稳定0.9.1及固定安装说明后形成[覆盖工具补齐方案](task6-coverage-tool-installation-proposal-2026-10-07.md)，已异步请求确认。授权未到达前不安装；此方案与之前API/SemVer/MSRV申请分开，也不授权系统安全配置操作。

完整C2无通过终态，完整C1随后执行；37源码子检查仍36PASS/1FAIL，旧compiler清单未改，完整wrapper自测此前五个stale manifest ERROR保留。Task6FIX_REQUIRED、Task7八义务、全部18顶层及API/SemVer/MSRV/features/网络/最终审查继续OPEN。

本轮只修改Python及文档。git diff --check实际通过，无Rust/依赖/分支/stage/commit/push/发布/系统安全配置更改。
