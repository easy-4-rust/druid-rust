# gRPC 采集恢复矩阵（10.2）

日期：2026-11-02
范围：`druid_metrics.v1` 双向流 + `druid-admin` 真实 TCP 边界；默认特性。
证据：`grpc-recovery-3x.log`、`workspace-test-per-package.log`。

## 执行结果

连续 3 轮 `cargo test -p druid-admin --test grpc_network_e2e_test`，每轮 **51 passed / 0 failed / 0 ignored**，退出码 0。测试内真实监听 `127.0.0.1:0` 并通过 tonic 客户端连接；不含 mock 传输。

## 场景映射

| 场景 | 代表测试 | 断言重点 |
|---|---|---|
| 断连后仅重发未 ACK frame | `grpc_client_reconnects_and_resends_only_pending_frame_bytes` | 已 ACK frame 不重发；pending 字节与 sequence 连续 |
| 完全 ACK 后重连保持下一 sequence | `grpc_reconnect_after_fully_acked_batch_keeps_next_sequence_and_updates_rest` | next sequence、REST 可见值 |
| 半关闭/重连命令幂等 | `reset_result_survives_real_tonic_half_close_and_reconnect_without_reexecution`、`final_command_reply_replays_after_half_close_and_duplicate_id_is_idempotent` | 命令只执行一次，回复可重放 |
| 重复/递增 sequence | `grpc_duplicate_and_increasing_sequences_are_handled_over_network` | 重复 ACK、单调 sequence、拒绝零/max |
| 乱序快照分片 | `out_of_order_snapshot_chunk_fails_closed_without_replacing_visible_state` | fail closed，不替换可见状态 |
| 无效 ACK 头 | `invalid_ack_headers_close_transport_and_preserve_pending_frame` | 关闭传输但保留 pending frame |
| 权威 full snapshot | `authoritative_snapshot_replaces_only_after_end_and_empty_snapshot_clears_stream`、`resync_required_supersedes_pending_with_monotonic_registry_full_snapshot` | END 前不生效、空快照清空、resync 单调 |
| resync 截止与 live 更新 | `resync_cutoff_preserves_live_update_queued_after_snapshot_sampling` | 采样后队列中的 live 更新保留 |
| Admin 重启 | `admin_restart_with_pending_incremental_supersedes_it_with_monotonic_full_snapshot`、`admin_restart_without_pending_triggers_registry_full_snapshot` | pending 被 full snapshot 取代；重启后重同步 |
| 流所有权/旧 epoch | `successor_stream_fences_old_epoch_before_it_can_overwrite_repository` | 新 epoch 先隔离旧流，旧流不能覆盖仓库 |
| 命令容量与背压 | `validated_w_plus_one_commands_survive_teardown_backpressure_and_live_traffic_resumes`、`two_w_plus_one_unique_commands_fail_closed_without_successor_execution` | W+1 背压、fail closed、后继流恢复 |
| 饱和停机 | `blocked_exporter_to_client_handoff_counts_each_batch_exactly_once`、`shutdown_timeout_reports_only_actual_pending_and_queued_work` | 批次 exactly-once、pending/queued 数值 |
| 未刷出 shutdown | `unavailable_admin_shutdown_reports_exporter_and_client_work_as_unflushed`、`runtime_shutdown_flushes_last_aggregated_batch_over_grpc` | 未刷出计数与最后批次 flush |
| Admin 关闭流 | `admin_shutdown_cancels_and_joins_every_owned_ingest_stream`、`admin_shutdown_releases_command_waiter_without_an_ack` | 流任务 join、命令 waiter 释放 |
| 重连 setup 期限 | `reconnect_rpc_setup_deadline_expires_and_the_supervisor_retries`、`reconnect_observation_excludes_setup_but_keeps_the_retry_deadline` | setup 过期、2s 故障窗口、后继连接 |

## 边界

- 本矩阵只覆盖默认特性。`--all-features` 下 `druid-admin` 的 gRPC 目标在当前 Windows 主机出现 `10060` 回环连接超时；同一测试默认特性 51/51 通过，已记录为环境/特性组合问题，未把它算作产品通过。
- 真实数据库/Admin K8s 部署不在本矩阵范围；10.3/10.4 另行验证。
