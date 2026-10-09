# Task7 标准 Blob 外部合同与候选（2026-10-08）

当前生产公开Blob仍是具体句柄alias，外部impl/dyn三处实际rustc E0404。Task7尚未完成，八项义务与18顶层任务不变。本次只在F证据目录准备候选与测试，未改冻结生产/正式测试/控制输入。

## 固定来源与语义修正

固定OpenJDK commit 7373384725418fa645fb4508ec05a869bc7fc3e7 的java.sql.Blob源码SHA256为6602719b09315b4b9d2c1866291a97660f188d85e0b907faa58643756465ae10，已与原69来源收据逐字节核对。源码包含11个JDBC方法声明，Blob不继承Wrapper。

getBytes返回至多请求长度的字节，不能把超过剩余长度统一认定为非法范围。初稿严格slice范围未运行，已在red2修正并新增请求99字节时返回剩余字节的断言，red1原始证据保留。按来源还保留1-based资源位置/0-based输入数组offset、搜索未找到-1映射None、free幂等及其他JDBC方法释放后SQLException。越过长度+1写入属于Java文档的驱动定义行为，夹具的明确拒绝不能推广为标准唯一行为。

[固定来源与red2收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-standard-blob-external-contract-red2.summary.json)

## 当前生产接口实际 RED

外部夹具使用Cell计数，保持Send但排除Sync；目标为Box<dyn Blob>与async &mut self。实际编译当前真实RDBC产物，impl Blob、pattern dyn Blob及Box dyn Blob三处均E0404。最新red3仍失败；没有进入其4项运行测试。该证据证明公开形态缺失，不证明方法实现或生命周期行为已认证。

## F盘独立候选实际验证

候选声明十一项JDBC方法和明确标注的Rust capability扩展，使用Send+'static而非Sync，所有async资源操作使用独占可变接收者，Blob模式参数同样接受可变标准trait object。所有方法为真实必需接口声明，未添加空函数或todo默认实现；候选尚未成为生产标准。

将同一外部合同改为引用该独立候选后，最新rustc --test -D warnings实际exit0且零输出；运行4PASS/0FAIL/0ignored：

1. 仅Send资源可由Box<dyn Blob>拥有，读写future满足Send。
2. 1-based读取、超长读取截取、非法读取/写入范围不改变数据、数组offset重载、独立Blob模式搜索与截断。
3. free幂等并只记录一次释放，释放后读写/截断拒绝。
4. 未声明STREAM时三种流方法明确SQLFeatureNotSupportedException；free后所有十种非free JDBC方法改为SQLException，释放优先于不支持；再次free仍幂等。

夹具中的字节修改/搜索/截断有真实逻辑；流能力明确未启用。此为外部实现夹具，不是数据库驱动，也不认证完整JDBC Blob数据类型支持。前三条对应初次候选3/0，最新第四条覆盖所有流方法并消除原dead_code警告，原日志保留。

[最新生产RED与候选4/0收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-standard-blob-candidate-contract2.summary.json)

[最新独立测试原始日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-standard-blob-candidate-contract2.log)

## 尚需完成与冻结

候选未接入druid-rdbc公共标准，现有access/context端口未退役，兼容句柄未迁移，驱动/Connection返回链和Druid owner包装未接入，原取消释放RED仍未修复。不得将候选4/0登记为生产Task7 GREEN或提升semantic inventory的LIVE状态。

全仓原54741仍在运行。standard-constant-exception-inputs.json的1104源码/58控制输入复核变化0，oracle/inventory未修改。原运行终态后再实施标准资源与完整owner仲裁，保留原行为、全部66测试实现候选与正式八义务范围。
