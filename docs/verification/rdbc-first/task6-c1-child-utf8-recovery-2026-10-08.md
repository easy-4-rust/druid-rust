# 完整 C1 子验证进程 UTF-8 恢复（2026-10-08）

## 真实失败

完整C1 runtime1/session28646已exit1。子C2使用默认Windows GBK解码Cargo输出，reader thread出现 `UnicodeDecodeError`，随后stderr为None导致拼接TypeError。该结果为工具执行失败，不证明产品断言失败；完整C1仍未通过。

原日志 `task6-complete-c1-runtime1-2026-10-08.log` 及同名summary保留。1092源码输入在终态逐文件一致；此前独立完整C2 runtime5的exit0证据不被改写。

## 复现、修复与回归

新增真实进程回归 `scripts/test_task6_child_utf8.py`：在调用环境明确设置 `PYTHONUTF8=0`，分别执行C1的runtime/structural C2启动函数，验证子进程真实 `sys.flags.utf8_mode` 与中文输出。修复前两条路径均观察到mode0，实际断言FAIL；修复后mode1且完整返回合同通过。测试夹具的PASS字符串是合成传输证据，不能作为真实1283测试迁移验收。

夹具第一次有字符串语法错误，日志 `task6-child-utf8-red-2026-10-08.log`保留；修正后实际mode RED为 `task6-child-utf8-mode-red-2026-10-08.log`，二者分开记录。

修复只改变 `verify_task6_method_parity.py` 两条C2调用的子进程启动：沿用当前 `sys.executable`，添加 `-X utf8`，调用者严格以UTF-8解码。没有ignore/replace错误字节，也没有放宽1283/479分母、成功标记、断言或7200/1800秒限制。

最终1项真实进程测试（包含runtime/structural两场景）PASS，原方法验证器22项自测PASS；不是将两个场景累加为两个独立测试。日志为 `task6-child-utf8-green-2026-10-08.log` 和 `task6-method-parity-utf8-regression-2026-10-08.log`。

## 完整 C1 重跑

确认原验证器及其子进程已退出后，启动默认实际C1 runtime2/session16743。未用`--no-execute`或跳过C2；同一Rust1.97.1/offline/locked/F盘target/单buildjob及testthread环境。

输入快照 `task6-complete-c1-runtime2-inputs-2026-10-08.json` 保留原1092源码输入，另冻结58项顶层Python工具与正式TSV控制输入。后续核验分别报告两种范围，不把控制输入混入历史1092分母。日志 `task6-complete-c1-runtime2-2026-10-08.log` 尚待终态。

Task6仍FIX_REQUIRED；编译覆盖、API/SemVer/MSRV/features、新鲜全仓回归、最终审查以及全部18顶层任务继续OPEN。没有安装工具、修改系统安全策略、切换分支、stage/commit/push或发布。

## runtime2 后续实际观察

原session16743保持存活：C1 Python50828于02:55:16启动，子C2 Python49664于02:56:57启动，实际子命令明确包含 `-X utf8`。02:59:57进入Druid外部消费者目标（Cargo34416），02:59:58测试二进制49572启动；通过进程树确认实际执行，没有重启或替换原运行。

再次逐文件检查1092源码输入与58工具/TSV控制输入，两组CHANGED均为空。尚未取得完整C1终态；当前未观察到新的解码错误不能替代最终通过证据。
