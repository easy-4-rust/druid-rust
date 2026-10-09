# Task6 参数 trace 与预处理 admission 源码门禁恢复

本次仅修改 Python 验证器、自测及证据文档，没有修改 Rust/Cargo/proto/SQL 构建输入。完整 C2 沿用 session13316；核查时 Python13276 与 wrapper all-features lib Cargo70556 均真实存活。运行期间1090项冻结输入逐项SHA核验一致。

## 参数绑定 trace

CodeGraph 当前源码显示两个真实生产入口：StandardAdapterPreparedStatement 和 SqlxPreparedStatement 的 set_input_parameter、set_parameter 各在成功写入参数状态后记录一次 task4_trace_parameter_set。旧文件级分母仍为一，导致真实合法源码不能通过，并遮蔽其他负向控制的诊断。

新增回归先对完整当前 wrapper 源码执行门禁，再把一个入口的事件移到另一个入口，使文件总数保持二。原门禁实际拒绝合法源码；完整42项自测初次终态 failures=4/errors=6/skipped=1，日志 task6-setter-trace-source-red-2026-10-07.log。修复将两个文件各精确登记二，同时将四个明确方法各精确登记一；无全局允许额外调用。

五项 trace 回归真实终态 PASS：合法双入口、相同文件总数下跨方法搬移、已有互斥分支、allowlist 外调用、helper/UFCS伪造与stage/validation anchor负向控制。日志 task6-setter-trace-targeted-green-2026-10-07.log。完整自测重跑42项仍 errors=5/skipped=1，五项都在加载过期 compiler coverage manifest 时失败；task6-setter-trace-source-green-2026-10-07.log 名称不代表完整套件通过。

## 预处理语句 admission

实际路径为 Connection::prepare_statement 以原 sql/options/GeneratedKeys::None 委托 prepare_statement_with_generated_keys；后者在缓存检查和原生取得之前 begin_admission，再将同一 admission 交给 DruidPooledPreparedStatement::new_standard。它是标准 trait 重载，不是任意私有 helper。

新回归实际 RED：合法当前源码因旧门禁只检查 prepare_statement 中的直接许可而失败。修复在同一 Connection impl 内核验生成键重载的 permit→raw acquisition→proxy ownership 顺序，并要求普通重载整个函数体等于原参数的单一委托表达式。

四项 admission 回归实际 GREEN，包括六种新负向变体：许可移动至raw之后、错误生成键模式、错误SQL、直接raw绕过、不可达委托、缺失目标重载；原create_statement许可后移、缺失producer、五个Statement许可身份控制仍有效。日志 task6-prepared-admission-source-red-2026-10-07.log 和 task6-prepared-admission-source-green-2026-10-07.log。

## 当前实际结果及未完成范围

同一37项源码子检查重跑：36 PASS/1 FAIL，唯一失败为编译覆盖清单源码hash过期。前次34/3日志与JSON保留，新证据为 task6-source-gates-after-setter-admission-2026-10-07.log/json。检查范围只有31 adapter来源、21 consumer、SQLx ownership、family executor、trace、coverage bindings和Druid admission；不能替代真实runtime、compiler、API或完整最终门禁。

编译覆盖必须重新运行七个family、105个精确函数身份生成并验证；不得仅替换hash。完整 C2 尚无通过终态，随后仍须完整 C1。API/SemVer/MSRV工具安装未获授权，不安装；网络11项失败及生产化认证继续开放。Task6仍FIX_REQUIRED，Task7八项义务及完整18个顶层任务均未关闭。

git diff --check 已实际通过；无分支/stage/commit/push/发布/系统安全配置变更。
