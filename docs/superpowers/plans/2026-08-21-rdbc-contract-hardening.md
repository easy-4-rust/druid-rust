# RDBC Contract Hardening Plan（已取代）

> **状态：SUPERSEDED**
>
> 原计划错误地把 `druid-rdbc::spi::Physical*` 和 runtime resource owner 作为最终 owner。
> 用户于 2026-08-24 明确否决该分层。
>
> 当前 P04 执行事实源：
> [`2026-08-24-rdbc-standard-bridge-realignment.md`](2026-08-24-rdbc-standard-bridge-realignment.md)
>
> 当前规格事实源：
> [`2026-08-24-jdbc42-standard-bridge-layering.md`](../specs/2026-08-24-jdbc42-standard-bridge-layering.md)

不得从本文件执行 Task 1–5，也不得据此继续在 `druid-rdbc::spi/resource` 中强化
capability、resource runtime 或 async cursor。

历史计划可通过 Git commit `2912808^` 之前的版本审计；其唯一仍有效的决策是：新 P04
Task 1 必须强制退出 transitional `compat::DruidError` waiver。
