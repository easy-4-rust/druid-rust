# 完整强类型 getter 与关闭案例推进（2026-10-06）

完整 getter BASE SHA-256 `96e097c165ea6ab8bad36a405668f4157b12dbab6df202847f6ff71179da7087`。通过原实际 SQLite Statement 的标准资源入场许可包装同样五列、一行 BufferedRowSet；保留 Decimal(1234567890.45)、2026-07-29、13:14:15.123456789、组合 timestamp、NULL，及四种Decimal读取、日期/时间/时间戳各四种下标/标签/省略/显式null/Shanghai重载、四列元数据类型和wasNull。原21assert_eq与2assert全部保留，85988实际终态exit0/1PASS0FAIL；既有标准实现满足完整合同，本例未修改生产逻辑。

正式精确绑定只更新一行台账后4字段与反向索引，只添加实际lib测试定位并移除对应矩阵占位标识，未改变验证器逻辑、分母或原断言。两个完整案例合计把本轮40个源码缺口减少至38；全1283原前7字段匹配，1245PASS38FAIL/1108exact175parameterized。C2 --no-execute仍exit1，首败connection_defaults_capture_compare_and_reset_complete_state；不能把源码观察比率称作完整迁移或行为等价率。

局部证据：完整关闭恢复Druid132/0与Wrapper26/0，另完整getter1/0（不与前lib97重叠）；尚待新鲜全workspace最终结果。新全仓62698实际cargo74560已核实LIVE，命令workspace/locked/offline/no-fail-fast/-j1/test-threads1，日志task6-close-and-strong-workspace-final.log。源码与Cargo输入保持冻结。所用stable rustc1.99.0，不是MSRV1.95或default1.97.1证据。前一全仓8243为3733/17、exit1，已保留终态记录。

15验证器自测、48final purity、79authority/25pending/73physical、14生成矩阵通过；derived migration document默认检查79通过。一次误用不存在--check参数为CLI预检查错误，已按实际支持默认检查重跑，不算产品失败或通过。

下一完整租约用例仅提取固定BASE，不映射、不运行、不绑定：完整状态8assert/6eq SHA0c7b4fe299716a981abf140005d6347f6e50299f54e23334f9e991557d826c71；完整SPI委托15assert/30eq SHA3a6e4ab75c9ebbb8eaa7ba58d4917a09413c83b4a5b08198e81d002826b07065。CodeGraph当前显示ConnectionDefaults.capture_standard硬编码holdability0/isolation2，standard_needs_reset/reset_standard_state不处理该两项，holder.reset忽略keep_transaction_isolation。holdability标准枚举目前仅1/2两个语义，原状态案例0需保留来源语义、不能直接把原0断言改成2。此为下一测试设计与实现的待验证事项，尚不声称完成或修复。

全部18顶层任务、Task6FIX_REQUIRED、Task7八义务、features/MSRV/API/SemVer/真实数据库/生产化/最终审查继续开放。
