# rdbc-jdbc-temporal-values Specification

## Purpose
明确 JDBC 时间值在本地时区、epoch 毫秒、纳秒与字符串之间的可观察转换规则，为跨语言数据库访问提供可复现的语义，并保留独立的 Rust 高精度值表达能力。

## Requirements

### Requirement: TV-01 Timestamp 保留时刻与本地字段的区别

JDBC 对齐的 Timestamp SHALL 按 Java 8 默认时区规则在 epoch 毫秒与本地日期时间字段之间转换；从本地字段构造后返回毫秒 SHALL 表示该时区下的实际时刻，不能固定按 UTC 重新解释。

#### Scenario: 上海时区的 epoch 零
- **WHEN** 默认时区为 Asia/Shanghai，使用 epoch 毫秒 0 构造 Timestamp
- **THEN** 本地字段为 1970-01-01 08:00:00，读取 epoch 毫秒仍为 0

#### Scenario: 默认时区变化后保留同一时刻
- **WHEN** 创建 Timestamp 后将进程默认时区从 UTC 改为 Asia/Shanghai
- **THEN** epoch 毫秒保持不变，后续本地字段读取按新的默认时区显示

#### Scenario: 上海本地零点
- **WHEN** 默认时区为 Asia/Shanghai，从字符串 1970-01-01 00:00:00 构造 Timestamp
- **THEN** 返回 epoch 毫秒 -28800000

#### Scenario: 夏令时歧义与负时间戳
- **WHEN** 在 UTC、Asia/Shanghai、America/New_York 执行负 epoch、DST gap 与 overlap 的冻结输入
- **THEN** 成功结果、归一化选择或失败结果逐项匹配项目冻结的 Java 8 oracle，不能因时区歧义自行选择通用拒绝策略

### Requirement: TV-02 Timestamp 纳秒与格式保真

Timestamp SHALL 保留 0 至 999999999 的纳秒，修改非法纳秒 SHALL 失败且保持旧值；字符串表示 SHALL 符合 Java 8 Timestamp 格式，整秒保留 `.0`，非零小数去除多余尾零。

#### Scenario: 整秒与小数格式
- **WHEN** 格式化相同本地字段、纳秒分别为 0、1 和 120000000 的 Timestamp
- **THEN** 秒后缀分别为 `.0`、`.000000001` 和 `.12`

#### Scenario: 无效纳秒不改变对象
- **WHEN** 已有有效 Timestamp 收到大于 999999999 的纳秒修改请求
- **THEN** 返回参数错误，原时刻和纳秒保持不变

### Requirement: TV-03 JDBC Time 的 LocalTime 转换舍弃纳秒

JDBC 对齐的 Time SHALL 在从 LocalTime 构造及转回 LocalTime 时只保留时、分、秒；该规则 SHALL 不改变独立 Rust 高精度 SQL TIME 值的能力。

#### Scenario: 带小数秒的输入
- **WHEN** 将 12:34:56.123456789 的 LocalTime 转为 JDBC Time 再转回
- **THEN** 结果为 12:34:56，且纳秒为零

#### Scenario: 高精度 Rust 值保持独立
- **WHEN** 调用方直接使用标准 Rust SQL TIME 值而没有经过 JDBC Time 包装转换
- **THEN** 不因本变更无条件截断其原有小数秒

### Requirement: TV-04 时间差分证据可复现

时间差分 SHALL 记录 Java 8 版本、冻结源码身份、时区和输入；不支持的表示范围 SHALL 明确报错或记录批准的 Rust 平台差异，不能溢出、panic 或将 JDK 17 的辅助实验冒充 Java 8 验收。

#### Scenario: 独立时区运行
- **WHEN** 并行验证不同默认时区
- **THEN** 每个时区使用隔离进程，结果互不受全局时区修改影响

#### Scenario: 超出表示范围
- **WHEN** 输入不能由目标 Rust 表示范围承载
- **THEN** 返回明确的范围错误，差异有方法级处置记录，不声明该输入已通过 Java 等价验证
