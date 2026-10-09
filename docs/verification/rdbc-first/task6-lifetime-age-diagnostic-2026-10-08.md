# Task6 池寿命与首次归还：受控年龄诊断（2026-10-08）

## 实际结果

使用当前正式Druid源码的Cargo构建产物，不使用Task7候选库。Rust/Cargo均显式固定1.97.1，新独立target为F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008。`cargo build -p druid --all-features --locked --offline --message-format=json-render-diagnostics` 实际exit0；外部诊断消费者`rustc --test --edition=2021 -Dwarnings`实际exit0。

同一消费者、不改变生产源码，控制运行3PASS/0FAIL；仅设置TASK6_LIFETIME_INJECT_BEFORE_CLOSE=1，在首次close前注入30ms调度延迟，原idle=1断言实际0PASS/1FAIL/exit101。

| 观察 | 无注入 | close前注入30ms |
| --- | --- | --- |
| close前连接年龄 | 34300ns | 34716600ns |
| close后连接年龄 | 122300ns | 35028900ns |
| max_lifetime | 10000000ns | 10000000ns |
| idle / active | 1 / 0 | 0 / 0 |
| discard / destroy | 0 / 0 | 1 / 1 |
| recycle / close | 1 / 1 | 0 / 1 |

另两个控制通过：将holder.created_at确定设置为一秒前，验证active0/idle0/discard1/destroy1/close1/recycle0、driver close一次及新借出不同holder；未过期控制采用独立3600秒寿命配置，验证同holder复用、只创建一次。后一配置只用于区分性控制，不替换原测试10ms配置。

## 判断及边界

CodeGraph源码显示PoolInner::return_connection在归还时检查connection_age >= max_lifetime，超龄直接淘汰；DruidPool借出空闲连接时也检查寿命。Java本地DruidDataSource recycle源码2019–2024同样具有phyTimeoutMillis超龄淘汰分支（该Java分支为严格大于，不能据此认证所有边界一致）。

本次证实：原测试首次归还必有idle=1的断言依赖归还前耗时小于10ms。受控延迟可复现相同idle0，且本次计数和物理释放没有泄漏。尚不能反推原全仓失败现场必由该延迟造成，因为历史现场没有年龄观测；本次也不是原迁移测试目标的正式修复、完整C1/C2或全仓GREEN。

下一步应把借出超龄idle分支的验证改为可控制年龄的内部测试夹具，保留原迁移身份、行为义务与独立归还超龄控制；不得删除归还寿命治理、延长原预算或放宽idle断言来制造通过。正式迁移前需核对固定BASE、映射及source-floor规则。

## 执行环境与证据

artifact1仅指定Cargo而环境rustc为1.99，E0514；artifact2显式rustc1.97.1但共享缓存已含1.99产物，仍E0514。两次原始日志保留，属于设置错误而非产品断言RED。artifact3使用全新独立target后成功，没有清理或删除旧缓存。

诊断summary绑定Cargo命令、四个精确依赖产物SHA、消费者SHA和两个实际运行退出码；helper、runner、compile/control/injected日志与summary共六文件逐字节镜像至SDD目录，前缀task6-e-space-。原1104源码和58控制输入再次核对变化0。未修改生产、原测试、迁移账本、断言、系统防火墙或工具安装。

当前完整workspace终态仍3876PASS/44FAIL；Task6 FIX_REQUIRED、Task7八义务、Task8 INCOMPLETE，18顶层任务保持OPEN。
