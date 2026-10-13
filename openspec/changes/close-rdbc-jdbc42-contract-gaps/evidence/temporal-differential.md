# JDBC 时间值差分证据

## 输入身份与运行器

- 对照类：`D:\codex_build_cache\rdbc_jdbc42_preflight_20261014\Jdbc42TimeOracle.java` 与 `Jdbc42ExtendedTimeOracle.java`，运行时及编译器均为 Oracle JDK 8u321-b07（`1.8.0_321`）。扩展 oracle 每次在独立进程内显式设置初始时区，再单独覆盖默认时区变化。原始 Java 源和 class 仅保存在 D 盘构建缓存；仓库仅保存文本输入/输出记录。
- 冻结参考 OpenJDK 8u502-b07 的源码 commit、归档 SHA-256、实际运行 JDK 补丁差异及 tzdb 版本见 [preparation.md](preparation.md)。本节是 8u321 实测，不冒充 8u502 执行证据。
- Rust 使用仓库锁定的 Jiff 0.2.35 时区数据库和系统时区解析。Windows 的 `chrono::Local` 不接受进程级 `TZ` 覆盖，因此 `Timestamp` 明确读取 `TZ` 并用 Jiff 数据库解析；未设置 `TZ` 时使用系统时区。时区变更后本地字段按新的 `TZ` 计算，瞬间身份不变。

## Java 8 原始输出：UTC

```text
zone=UTC
epoch0.local=1970-01-01T00:00
epoch0.millis=0
localZero.millis=0
timestamp.zero.format=1970-01-01 00:00:00.0
timestamp.nano.format=1970-01-01 00:00:00.000000001
timestamp.fraction.format=1970-01-01 00:00:00.12
time.local=12:34:56
time.nano=0
dst.2024-03-10 02:30:00.millis=1710037800000
dst.2024-03-10 02:30:00.local=2024-03-10T02:30
dst.2024-11-03 01:30:00.millis=1730597400000
dst.2024-11-03 01:30:00.local=2024-11-03T01:30
```

## Java 8 原始输出：Asia/Shanghai

```text
zone=Asia/Shanghai
epoch0.local=1970-01-01T08:00
epoch0.millis=0
localZero.millis=-28800000
timestamp.zero.format=1970-01-01 00:00:00.0
timestamp.nano.format=1970-01-01 00:00:00.000000001
timestamp.fraction.format=1970-01-01 00:00:00.12
time.local=12:34:56
time.nano=0
dst.2024-03-10 02:30:00.millis=1710009000000
dst.2024-03-10 02:30:00.local=2024-03-10T02:30
dst.2024-11-03 01:30:00.millis=1730568600000
dst.2024-11-03 01:30:00.local=2024-11-03T01:30
```

## Java 8 原始输出：America/New_York

```text
zone=America/New_York
epoch0.local=1969-12-31T19:00
epoch0.millis=0
localZero.millis=18000000
timestamp.zero.format=1970-01-01 00:00:00.0
timestamp.nano.format=1970-01-01 00:00:00.000000001
timestamp.fraction.format=1970-01-01 00:00:00.12
time.local=12:34:56
time.nano=0
dst.2024-03-10 02:30:00.millis=1710055800000
dst.2024-03-10 02:30:00.local=2024-03-10T03:30
dst.2024-11-03 01:30:00.millis=1730615400000
dst.2024-11-03 01:30:00.local=2024-11-03T01:30
```

## Java 8 原始输出：默认时区变化及边界

```text
timezone.initial.local=1970-01-01T00:00
timezone.changed.local=1970-01-01T08:00
timezone.changed.millis=0
negative.local=1970-01-01T07:59:59.999
range.-9223372036854775808.millis=-9223372036854775808
range.-9223372036854775808.local=+292278994-08-17T15:12:55.192
range.9223372036854775807.millis=9223372036854775807
range.9223372036854775807.local=+292278994-08-17T15:12:55.807
```

这确认 `Timestamp(long)` 的内部瞬间不会因默认时区变化而改变，而 `toLocalDateTime()` 使用调用时的默认时区。Rust 因此保存 epoch 秒/纳秒作为身份，并动态读取本地字段；Chrono 的日期表示范围小于 Java `long` 毫秒全范围，对超范围值返回 `InvalidArgument`。

## Rust 验证

先用新增用例运行旧实现，5 项中 3 项按预期失败：epoch 0 本地字段、整秒 `.0` 格式、JDBC `Time` 纳秒截断。修复后：

- `cargo test -p druid-rdbc --test jdbc_temporal_value_contract_test --locked --offline`：8 passed，包含四个隔离子进程。
- `cargo test -p druid-rdbc --test standard_sql_type_map_contract_test --locked --offline`：5 passed。
- `cargo test -p druid-rdbc --locked --offline`：完整 crate 和 doc tests 通过。
- `cargo fmt -p druid-rdbc -- --check`：通过。

Rust `jdbc_temporal_value_contract_test` 在独立子进程中分别设置 `TZ=UTC`、`TZ=Asia/Shanghai`、`TZ=America/New_York`，逐项对照上方 Java epoch 0、本地零点、DST gap 与 overlap 毫秒；另一个隔离子进程在运行中将 `TZ` 从 UTC 改为上海，验证既有 Timestamp 的 epoch 毫秒不变而本地字段更新。以上 Rust 差分用例全部通过。Windows“设置”界面修改系统时区而不设置 `TZ` 的刷新时延由 Jiff 系统检测缓存控制（最多数分钟），本批未通过全局系统设置变更进行验证。

## 迁移说明

- `Timestamp::from_local_date_time` 现在返回 `Result<Timestamp, RdbcError>`，让 DST/范围解析失败可见；调用方需处理 `?` 或明确映射错误。
- `Timestamp::millis` 现在返回 `Result<i64, RdbcError>`，防止平台时间范围或毫秒溢出被静默替换。
- `Timestamp::from_millis` 会保留绝对时刻；`to_local_date_time` 按当前本地时区反算字段；相等、排序、哈希按时刻及纳秒定义。
- JDBC `Time::from_local_time` 会舍弃小数秒；直接创建 `Value::Time(NaiveTime)` 仍保留 Rust 高精度值。
- `druid-rdbc` 新增 Jiff 作为直接依赖，用于跨平台 IANA 时区规则与进程 `TZ` 覆盖；依赖版本已由工作区锁文件固定。显式设置 `TZ` 时按每次转换读取新值；未设置时使用平台系统时区检测。
