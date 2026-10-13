# CD-02/04：数据源安全诊断与故障隔离

## 结果

7.3 已完成。数据源失败诊断现在使用数据源实例自己的 writer；公共诊断输出只包含固定操作名、错误类别和格式校验后的 SQLState，不输出原始错误消息、URL、属性、用户名、密码、token 或凭据。操作名被限制为短 ASCII 标识。日志接收器错误只暴露安全的 I/O 错误类别，接收器提供的任意文本不会回显。

池化 Druid、普通 RDBC wrapper 和 XA 数据源分别在其 owner 边界发出诊断。日志故障不会替换原连接/配置错误，也不会改变成功连接结果或 XA 配置属性。`RdbcError::sql_exception()` 沿池和数据源包装错误继续查找原始 SQL 异常，使外层包装不丢失 SQLState。

## 合同与验证

- `cargo test -p druid-rdbc --test common_data_source_logging_contract_test --locked --offline`：4 passed。验证安全上下文、凭据脱敏、接收器 I/O 错误不回显，以及三类标准数据源共享合同。
- `cargo test -p druid --test common_data_source_logging_pool_test --locked --offline`：4 passed。验证池化连接 SQL 故障、日志 I/O 故障同时发生时保留原 SQL 错误和配置；并验证实例间隔离及不污染 `DriverManager` 全局 writer。
- `cargo test -p druid-wrapper --test common_data_source_logging_test --locked --offline`：2 passed。验证普通 wrapper 凭据拒绝时日志保留固定诊断上下文且不泄露尝试凭据，以及标准配置合同。
- `cargo test -p druid-wrapper --lib --features jdbc-agent jdbc_agent::jdbc_agent_xa_data_source::tests:: --locked --offline`：9 passed。验证 XA owner 诊断脱敏、原 SQL 错误精确保留、sink 故障不改变成功连接、XA 属性保持及 writer 状态。
- `cargo test -p druid --test druid_error_transition_test --locked --offline`：4 passed。验证包装错误/SQL 错误转换回归。
- 上述命令均在本地实际执行并退出码为 0；均使用锁文件和离线依赖。

## 观察到的边界

尝试在真实池化数据源集成夹具中加入成功连接场景时，连接获取在本机夹具中超过 60 秒未结束，因此停止该额外探针并移除。池化失败路径和 sink 故障隔离由稳定的故障注入测试验证；XA 测试另有成功连接加 writer 故障场景并通过。该夹具限制不作为池化成功连接的验证证据，也不影响本项对池化诊断错误路径的验收。

## 源码身份（SHA-256）

```text
crates/druid-rdbc/src/data_source/common_data_source.rs 45BFE2AFAFE48B960C2CBA65641DAD91B37CE896753188C82652BF83B3BD5EE6
crates/druid-rdbc/src/data_source/mod.rs 2C46780933DB990B3B558B90AF16195942B5AFC5BA253B846F4A738E5C1E9C49
crates/druid-rdbc/src/error/rdbc_error.rs 85761C9640EC7583EF0DE3EBF884E53939B4FD4E3D9EA7B31EE13158BFFEF0F7
crates/druid-rdbc/tests/common_data_source_logging_contract_test.rs E6846EE15677246B04DB40ED475A835E890CF4AEB973C929B30E66D6C89B620D
crates/druid/src/pool/druid_data_source.rs FCF91A014FD574811B4012C61F79AFCF312474263A995214EB3F73BC6A4E296D
crates/druid/src/sql/data_source.rs 2B5D0F13116C33BE23D0DD3D29BD90B0115A118A588262B9A7DB986FE3CDE756
crates/druid/tests/common_data_source_logging_pool_test.rs B3208941C411C58558968136592728127D42D3F99E80E3C28EC94027F1A89DC6
crates/druid-wrapper/Cargo.toml 5D76346552145038001A85027A4D1627EE7D5C8D44B58AC9AA6BFD622F2DE0F3
crates/druid-wrapper/src/rdbc/druid_rdbc_data_source.rs 7CB51207FCBB2B7C7B8BA05E1997628B09D445779EBFD8542B8DD37B56E0D5F9
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_data_source.rs F9D0AABCE9C44B5D48F0CAE9072E71C1C394951F3F4B498AE3791724E82D1780
crates/druid-wrapper/tests/common_data_source_logging_test.rs C89D3F17B3F1AECCE02B066213170B94BAD648476B672B91414152E450B18F7B
Cargo.lock 531CA88E2F113CA0D034A7778AADCBAE499E76091F137F9BD791A94E7BA13560
```

文件：

- `crates/druid-rdbc/src/data_source/common_data_source.rs`
- `crates/druid-rdbc/src/data_source/mod.rs`
- `crates/druid-rdbc/src/error/rdbc_error.rs`
- `crates/druid-rdbc/tests/common_data_source_logging_contract_test.rs`
- `crates/druid/src/pool/druid_data_source.rs`
- `crates/druid/src/sql/data_source.rs`
- `crates/druid/tests/common_data_source_logging_pool_test.rs`
- `crates/druid-wrapper/Cargo.toml`
- `crates/druid-wrapper/src/rdbc/druid_rdbc_data_source.rs`
- `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_data_source.rs`
- `crates/druid-wrapper/tests/common_data_source_logging_test.rs`
- `Cargo.lock`
