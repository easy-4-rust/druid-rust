# 异常分类器配置的 Java 整数语义

## Why
当前 Java `AbstractOracleExceptionSorter#configFromProperties` 委托 `Integer.parseInt`，接受 BMP 十进制数字；当前 Rust `item.parse::<i32>()` 只接受 ASCII 数字。实跑当前 Java 源码已确认全角/阿拉伯文配置生效，而 Rust 会丢失这些致命错误码。

## What Changes
- 仅修复共享 Oracle 配置解析，保留逗号切分、不 trim、忽略非法值、追加去重以及 setter 替换语义。
- 复用原 576 条冻结异常语料、Java oracle、原始源测试及池处置测试；补充独立 Unicode/溢出边界语料，不改既有 approved difference。
- 对该批分类器逐方法登记证据，运行 Java/Rust 差分和 cargo-llvm-cov。构建与临时文件全部放 D:。

## Impact
无公共 API/依赖/拓扑变化，不修改并行 Admin 工作。整仓迁移状态仍未完成。
