# Filter 类身份迁移修正

## Why
Java fa8dc9912637a2f729eef9f55356621fec18d40e 的 FilterManager.existsFilter 使用 String.equals；Rust b7c2afc59cf67e11f2d66fe7769093ca2d7f93f9 仍使用 eq_ignore_ascii_case。两个已注册且仅大小写不同的类会被错误合并。

## What Changes
- 按精确类名去重；重复同名仍只构造一次。
- 保留别名展开顺序、未知工厂跳过和构造异常传播。
- 更新旧测试误写的 equalsIgnoreCase 说明，并增加两个真实注册工厂的回归矩阵。

## Impact
仅影响 FilterChain 的类身份判断，不调整 crate 拓扑、不改用户 Admin 工作区。整体迁移仍未完成。
