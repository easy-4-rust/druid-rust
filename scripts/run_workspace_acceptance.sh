#!/usr/bin/env bash
# 整仓验收：逐包运行 Rust 测试，再生成来源用例逐条状态。
#
# 逐个 crate 运行而不是单次 `cargo test --workspace`，是为了隔离 Windows
# 回环端口压力；每个包仍是完整测试目标，失败会保留退出码并使总门禁失败。
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/docs/verification/migration-completion-20261102"
LOG="$OUT/workspace-acceptance-cargo.log"
PACKAGES=(druid druid-rdbc druid-admin druid-wrapper druid-metrics druid-bench druid-test)

mkdir -p "$OUT"
: > "$LOG"
overall=0

for package in "${PACKAGES[@]}"; do
  printf '===== PACKAGE %s =====\n' "$package" | tee -a "$LOG"
  cargo test -p "$package" 2>&1 | tee -a "$LOG"
  status=${PIPESTATUS[0]}
  printf 'PACKAGE_%s_EXIT=%s\n' "$package" "$status" | tee -a "$LOG"
  if [ "$status" -ne 0 ]; then
    overall=1
  fi
done

printf 'ACCEPTANCE_CARGO_OVERALL_EXIT=%s\n' "$overall" | tee -a "$LOG"
cargo run -p druid-test --bin source_parity_acceptance 2>&1 | tee -a "$LOG"
report_status=${PIPESTATUS[0]}
printf 'ACCEPTANCE_REPORT_EXIT=%s\n' "$report_status" | tee -a "$LOG"

if [ "$overall" -ne 0 ] || [ "$report_status" -ne 0 ]; then
  exit 1
fi
