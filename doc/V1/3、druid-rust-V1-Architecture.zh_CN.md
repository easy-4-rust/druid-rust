# druid-rust V1 Architecture（zh_CN，链接入口）

> **文档说明**：V1 版本级架构细化以仓库根的
> [`druid-rust-Architecture.zh_CN.md`](../../druid-rust-Architecture.zh_CN.md)
> 为唯一来源；本文件作为引用入口，避免多处维护。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | 版本级架构（链接入口） |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

## 2. V1 范围对应的架构章节

完整架构见仓库根 `druid-rust-Architecture.zh_CN.md`。V1 范围对应章节：

| 章节 | 主题 | 与 V1 的关系 |
| :---: | :--- | :--- |
| §6 | ADR | V1 受 ADR-001 ~ ADR-005 约束 |
| §8 | 组件清单 | V1 实现 `druid-core`、`druid-sql`、`druid-pool` |
| §9 | 运行时与并发 | V1 在 mock driver 上验证 |
| §10 | 主链 | V1 验证 `Pool::get → FilterChain::before → Connection::exec → Drop 归还` |
| §11 | 状态机 | V1 验证 Idle/Active/Closed 转换 |
| §15 | 安全 | V1 验证 Wall 默认 deny |
| §23 | 实施路线 | V1 退出条件见 §4 |

## 3. V1 退出条件（来自根架构 §23）

| 阶段 | 交付物 | 退出条件 | 依赖 | 回退 |
| :--- | :--- | :--- | :--- | :--- |
| Phase 1 | `druid-core` + `druid-sql` + `druid-pool` + mock driver | `SELECT 1` 端到端；Wall 拦截 `DROP TABLE` | Phase 0 | 退回 `deadpool` 直用 |

## 4. V1 不实现的章节（待 V2/V3）

- §12 数据一致性中关于 SQL 合并统计的细节（V2）。
- §13 协议中 `/druid/admin/*` HTTP 端点（V3）。
- §17 性能预算中关于多数据源的指标（V3）。
- §18 部署中独立进程的 admin 服务（V3）。
- §19 可观测性中的 Prometheus 抓取配置（V2）。
- §20 扩展中 `LoadBalancer` trait 细节（V3）。

## 5. 一致性自检清单

- [ ] 本文件不重复根架构内容。
- [ ] V1 退出条件与根 §23 一致。
- [ ] V1 范围与 [V1/4、功能规划](4、druid-rust-功能与界面规划-V1.md) 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审