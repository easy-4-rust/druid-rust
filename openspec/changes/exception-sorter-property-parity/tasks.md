## 实施与验证
- [x] 冻结源码、现有方法/用例清单与并行修改基线
- [x] 增加独立配置边界语料及共享 oracle 入口，确认 RED
- [x] 修复共享配置解析，不改变既有分类规则
- [x] 统一运行源测试、原有/补充 live differential、Rust 及池处置回归
- [x] 运行 cargo-llvm-cov，记录未覆盖分支和门禁限制
- [x] 更新逐方法台账及四份迁移主文档，检查并行修改漂移
- [ ] 完整验收门禁闭合（严格 Clippy 被既存 shared_row_set 通配导入阻断；全量验收未执行，不归档）

证据：[逐方法报告](../../../docs/verification/method-semantics-20261101/README.md)。原576差分为573 MATCH+3既有批准差异，补充118全部MATCH；普通Rust17通过、分类harness4通过、冻结Mock harness2通过；当前Java vendor18原方法通过，当前Mock原断言/观测通过。不得据此授予整个对象/全仓迁移完成。

源测试基线限制：旧Mock harness直接指向当前Jupiter测试文件时哈希失败；保留原哈希，独立重建冻结资产并另执行当前原测试。Java池依赖1.2.28 jar，不是全池当前源码构建。其他语义（日志/Locale/宿主配置/集合别名）仍见报告待办，未扩张为本变更已完成项。
