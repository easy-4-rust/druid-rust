# 验证记录与交付边界

> 2026-11-02 状态更新：修订 04 已获批准并落实到生产 Vue / Rust。本文件保留设计阶段的范围与基线记录；当前实现、验证及尚未接入指标以 [生产落地记录](../../verification/admin-live-console-20261102.md) 和 OpenSpec `admin-live-console` 为准。
日期：2026-11-01；Rust 基线 `ed161654c203f60e87ea11b4524ca57496a10e73`。本轮仅新增设计/原型/验证材料和现有文档入口，未修改产品 Rust / Vue 实现。

## 当前交付：主页与详细监控修订 04

保留 revision 03 的顶部导航、紧凑表格与行内直方图；新增默认主页，补齐数据源三标签、数据源详情四标签、SQL 详情三标签、Wall 五标签、慢 SQL 阈值、采集健康、能力矩阵和 API 参数说明。

通过 CUA 在预览浏览器执行 **59 项检查，59 项通过**：24 项八页面×三尺寸布局，8 项数据源标签/详情/精确下钻，10 项 SQL 详情/Wall/缺能力交互，4 项阈值/分页/排序/搜索刷新，5 项主页异常状态，6 项最终 Wall/API 内容三尺寸复验，1 项 Escape，1 项默认入口。布局检查确认页面本身不横溢出，宽表在容器内滚动；截图人工检查主页桌面和手机效果。最终检查阶段无新增浏览器错误。开发过程中发现并修复了字符串模板与直方图变量错误，未将修复前结果计为通过。

原始记录：[middleware-v4-checks.json](evidence/middleware-v4-checks.json)。截图位于 `D:\codex-build\druid-admin-design-20261101\visual-v4\`，包含主页桌面/平板/手机、生命周期及 SQL 详情。最后恢复临时视口尺寸，预览停留主页。

本轮仅改原型和设计材料；没有重跑 Rust 测试或覆盖率。下方 24 项后端测试与 26.91% scoped 覆盖仍为先前基线，不是新增验证，也不满足整仓迁移覆盖门禁。复合身份、数据可信度、真实链路与完整字段回归仍按 remaining-work 台账执行。

## 历史：原稿风格修订 03

用户以原稿截图明确了中间件控制台风格，本轮撤销前两版通用仪表盘布局，恢复顶部导航、环境摘要、紧凑 SQL 表格与四组行内直方图。读取现有 SqlList / MiniHistogram / format 实现，保留分桶颜色和累计口径。

本轮通过 CUA 在用户预览浏览器验证 **33 项检查**：8 个页面×3 个尺寸共 24 项布局检查；10/2 行分页、错误排序且 unknown 置后、搜索后刷新、billing 同号 SQL 复合身份 4 项交互检查；部分/过期/空/失败/无权限 5 项状态检查。SQL 初始表观察为 12 行、44 组已支持直方图，另 1 条 push SQL 的 4 组分布明确不支持。检查结束恢复窗口尺寸，用户预览停留 SQL 统计页。

历史记录：[middleware-v3-checks.json](evidence/middleware-v3-checks.json)。截图：`D:\codex-build\druid-admin-design-20261101\visual-v3\sql-console.png`。本轮没有重跑 Rust 覆盖率；下方 24 项后端测试与 26.91% scoped 覆盖是原基线，不能算成这轮新增通过。下方 revision 01/02 的 35 项记录与脚本保留为历史，不适用于当前已移除的趋势首页。没有宣称全量迁移或生产前端验收完成。

## 1. 后端基线：复用现有测试

使用现有工具链与依赖，未安装组件。临时目录、构建和 coverage 输出均在 D:。

实测工具：`rustc 1.97.1 (8bab26f4f 2026-07-14)`、`cargo-llvm-cov 0.9.1`。

```powershell
# 已存在的环境脚本：TEMP/TMP、无调试信息、关闭 incremental 等
. D:\codex-build\druid-sorter-methods-20261101\environment.ps1
$env:CARGO_LLVM_COV_TARGET_DIR = 'D:\codex-build\druid-admin-design-coverage-20261101'
cargo llvm-cov --offline --locked -p druid-admin `
  --test admin_v1_api_test --test admin_console_routes_test `
  --test admin_poll_backend_test --test admin_trace_redaction_test --summary-only
cargo llvm-cov report -p druid-admin --json `
  --output-path D:\codex-build\druid-admin-design-20261101\admin-coverage.json
```

| 测试目标 | 本轮结果 |
|---|---:|
| admin_console_routes_test | 9 passed |
| admin_poll_backend_test | 5 passed |
| admin_trace_redaction_test | 1 passed |
| admin_v1_api_test | 9 passed |
| 合计 | **24 passed，0 failed，0 ignored** |

| llvm-cov 指标 | 覆盖 / 分母 | 比例 |
|---|---:|---:|
| 行 | 1,395 / 5,183 | **26.91%** |
| 区域 | 1,886 / 7,141 | 26.41% |
| 函数 | 184 / 636 | 28.93% |
| 分支 | 本次未采集 | 不报告通过率 |

范围：`-p druid-admin`，以上四个 integration test targets，默认 feature，当前 Windows 主机；不是 `--workspace --all-features`。TOTAL 是该命令纳入的对象文件/源码分母，不是 Vue 覆盖率、全工作区覆盖率或 Java 语义完成率。本轮没有重新执行 Java 全套源测试/资源一致性、全驱动宿主矩阵、MSRV、全量 Clippy、故障预算或变异测试，**不能宣称满足迁移测试技能的最终门禁**。本轮无生产代码修改，不伪造新的生产 TDD RED/GREEN 记录。

原始测试日志：[admin-coverage.log](evidence/admin-coverage.log)；结构化覆盖摘要：[coverage-summary.json](evidence/coverage-summary.json)。完整 JSON 留在 `D:\codex-build\druid-admin-design-20261101\admin-coverage.json`，摘要记录其 SHA-256。

过程异常也保留说明：第一次导出 JSON 时把 `CARGO_TARGET_DIR` 设置为 coverage 目录，report 误找其下 `llvm-cov-target`，因找不到 profraw 失败。随后使用与测试一致的 `CARGO_LLVM_COV_TARGET_DIR` 成功导出；没有通过修改统计范围隐藏失败，也没有重跑测试改变分母。

## 2. 原型交互与视觉验证

视觉修订 02：用户反馈首版外观不足后，重做视觉与信息层级。现有 35 项浏览器检查重新执行通过；截图位于 `D:\codex-build\druid-admin-design-20261101\visual-v2\`。当前 [prototype-checks.json](evidence/prototype-checks.json) 对应 revision 02；首版结果保留为 [prototype-checks-v1.json](evidence/prototype-checks-v1.json)。本轮没有修改 Rust 代码，也没有重跑或更新上一节后端覆盖率。桌面和手机截图人工复查，并刷新用户正在看的原型页面。

上述 35 项检查之后，仅恢复了全局采样提示的可见性；最终文件又在用户预览浏览器中检查 1280×1024、768×1024、390×884，均无页面横溢出，采样提示可见。随后恢复原窗口尺寸。两阶段源码哈希分别记录，见 [最终布局检查](evidence/visual-v2-final-layout.json)，不把较早截图冒充最后一行 CSS 调整后的截图。

Playwright 使用机器已安装的 Chrome，无下载或依赖安装。初次默认 Chromium 启动因缓存中没有对应浏览器可执行文件而失败，改用已安装 Chrome 后执行；没有按错误提示擅自安装。

- **35 项断言通过**，0 个浏览器 JavaScript 错误。
- 三个尺寸：Desktop **1280×1024**、Tablet **768×1024**、Mobile **390×884**。每个尺寸遍历七个导航视图，确认存在内容且页面不横向溢出；宽表只在自身容器横滚。
- 验证服务筛选、同号 SQL 的复合身份、push 缺失字段、全缺失不显示零、Wall 来源范围、搜索、刷新保留搜索、暂停、趋势窗口、五种故障/权限/空状态、恢复、移动端抽屉、Esc 关闭与焦点归还。
- 保存桌面、平板、手机和手机详情截图；人工检查桌面与手机总览的布局、信息层级和溢出。没有把人工浏览声称为完整 WCAG / 屏幕阅读器审核。
- fixture 状态与同名生产 API 状态没有自动等价关系；原型未发网络请求，刷新只更新演示时钟和提示。完整 SQL 仅为兼容模式的虚构示例，secure 模式要求见 AC13。

首两版历史检查脚本（不适用于 revision 03）：[verify-prototype.cjs](evidence/verify-prototype.cjs)，结果：[prototype-checks.json](evidence/prototype-checks.json)。脚本默认使用本机已安装路径；可用 `DRUID_PLAYWRIGHT_MODULE`、`DRUID_BROWSER_PATH` 和 `DRUID_PROTOTYPE_OUTPUT` 指向现有环境，输出目录应保留在 D:。

截图位置：`D:\codex-build\druid-admin-design-20261101\{desktop,tablet,mobile,mobile-detail}.png`。这些是原型图，不是当前生产界面的截图。

## 3. 独立设计评审

依照设计流程进行了只读 fresh builder-check：首次发现 secure SQL 合同歧义、全缺失错误值显示零、来源范围不一致、可选 Viewer 被列为已确认缺口、字号/点击高度不一致。五项均已修订并经独立静态复核关闭。Viewer 现为 N01 可选建议；安全模式要求有 AC13 明确验收。

评审者没有重复运行浏览器或后端测试；运行证据来自本主任务。此评审没有批准新增生产 API 或改变现行规格。

## 4. 文档完整性与后续

使用单模块评审包，未初始化新的规格体系：README → 剩余工作 → 页面/状态/指标/验收设计 → 原型 → 验证。design.md 的 P01–P10 页面与 S01–S04 子界面映射到 A1–A4/R3；AC01–AC13 对应可观察行为。新建文件的相对 Markdown 链接、源码依据和产物哈希在收尾检查中校验。

源码快照：[source-sha256.json](evidence/source-sha256.json)。产物校验：[evidence-sha256.json](evidence/evidence-sha256.json)。该快照覆盖本设计用到的主要源码和四个测试，不冒充全仓逐方法台账。

当前可交付的是设计与原型。下一步为评审设计后按既有 OpenSpec 建立 A1 增量 change，先修复复合身份、数据质量、慢 SQL 口径和有界查询，再落地 Vue 视觉；趋势、遥测、审计按 A3/A4 验收后补齐。生产迁移状态保持“尚未全量验收”。
