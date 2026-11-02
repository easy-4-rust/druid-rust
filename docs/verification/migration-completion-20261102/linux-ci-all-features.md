# Linux CI 全量默认/all-features 测试证据（1.5）

日期：2026-11-02
CI：run `41396400600`，job `rust-contracts`（`125118552514`，ubuntu-latest），commit `5a6f6941`。

## 结果

| 命令 | 测试二进制数 | passed | failed | ignored |
|---|---:|---:|---:|---:|
| `cargo test --workspace` | 372 | **4435** | **0** | 12 |
| `cargo test --workspace --all-features` | 382 | **4619** | **0** | 16 |

同一 job 的以下步骤也全部通过：

- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings -A clippy::pedantic`
- `cargo doc --workspace --all-features --no-deps`
- `python3 scripts/verify_task7_resource_operations_closure.py`
- `python3 scripts/verify_druid_single_standard.py`

job 结论：**success**。

## 证据提取

```bash
gh run view --job 125118552514 --log > rust-contracts-ci.log
# 按 step 前缀过滤并累加 "test result:" 行
```

计数规则：每行 `test result: ok/FAILED. N passed; M failed; K ignored` 计入对应 step；默认与 all-features 分开统计，未把 ignored 计为通过。

## 与本地 Windows 的关系

本地 Windows 的 `--all-features` gRPC 目标曾出现 `10060` 回环超时；Linux CI 同一 `cargo test --workspace --all-features` 运行 4619 passed、0 failed，包含 `grpc_network_e2e_test` 51/51。因此 1.5 以 Linux CI 全量结果为准，本地异常保留为环境说明，不再阻塞任务。

## 边界

- 该证据覆盖工作区所有测试目标，不代表 SOURCE_PARITY 4759 用例已映射；2.3 仍为 `MISSING/NOT_RUN`。
- ignored 16 项未计入通过；其中包含需要真实数据库/外部宿主的场景，归属 9.x。
