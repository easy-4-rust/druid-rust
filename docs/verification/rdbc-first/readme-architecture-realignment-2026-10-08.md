# 中英文 README 架构与证据导航校正（2026-10-08）

对应正式规格：ADR-CRATE-002 的 RDBC-first 五产品架构，以及既有唯一 standard trait tree。此处修正文档的当前事实，不修改规格或验收标准，不关闭 P04 Task8/P07 文档门禁。

## 修正内容

- 将旧三产品源码/druid-core+facade目标描述改为实际五产品 crate + druid-bench 验证成员，说明 ADR-CRATE-002 已取代 ADR-CRATE-001。
- 使用 Mermaid 展示实际依赖方向，包括 Admin 对 Wrapper 的可选依赖。
- 使用标准 Connection、ConnectionFactory、ExternalConnectionLease、ConnectionCapabilities 描述当前链路；保留 Physical 名称作为迁移历史，不再称标准 Connection 为非 canonical 兼容名称。
- 校正 Toasty 与 druid-driver 工具归属为 druid-wrapper，工具示例显式选择 managed-driver-install feature。
- 删除不存在的 druid metrics/wrapper feature 用法；按当前 manifests区分空数据库feature声明、Wrapper真实driver feature与Metrics client/server。
- 明确 SQLx/RBDC/bb8/deadpool 当前仍是非optional依赖，不将P05目标写成已实现。
- 将 Admin 占位描述改为实际 Topcoat/Axum 与 HTTP/gRPC 服务，同时保留网络验收开放状态。
- 校正五产品 package 发布目标，保留全部publish=false及发布前验收条件。
- 替换七个已删除测试入口，链接到当前 canonical矩阵、外部Wrapper合同、Filter统计与prepared回归；链接存在不视为运行通过。
- 删除不存在的 docs/README.md 与旧迁移总路线图入口，指向现有正式主路线、规格与验证账本。
- 去除未标日期的构建/全仓通过或旧core断言故障描述，统一引用当前验收证据，避免另一份完成率账本。

## 实际核验

初次全docs链接核验失败：两语言均存在旧迁移总路线图与docs/README.md失效链接。随后扩大只读链接核验，发现相同七个已删除测试入口，依据当前CodeGraph源码与迁移记录校正。

更新脚本两次因精确原文标签不匹配拒绝落盘；两个语言先全部匹配成功才写入，失败期间未单边改动README。修正匹配后实际更新成功。

最终独立检查exit0：两语言所有相对文件链接存在、代码块闭合、示例Cargo feature在对应manifest声明、示例binary属于对应package。git diff --check实际exit0。未把这些文档检查记作Rust编译/运行测试。

原runtime3快照1090项Rust/Cargo/proto/SQL构建输入逐文件SHA核验无变化。全功能wrapper lib目标原session45521/Cargo46440继续真实运行；已观察原DuckDB编译结束并推进其他依赖，尚未取得整个目标通过证据。日志task6-wrapper-all-features-prebuild-2026-10-08.log。

## 尚未完成

README导航校正不等于Task8全量文档/API/差分/实库验收。架构基线和历史规格仍需结合日期解读；后续新鲜完整门禁取得终态后继续对照正式要求复核所有文档、接口和性能说明。

Task6保持FIX_REQUIRED；完整C1/C2、compiler coverage、API/SemVer/MSRV/features、当前全仓与网络、最终审查仍开放；Task7八项及全部18顶层任务未关闭。未安装工具、变更分支、stage/commit/push或修改系统安全策略。
