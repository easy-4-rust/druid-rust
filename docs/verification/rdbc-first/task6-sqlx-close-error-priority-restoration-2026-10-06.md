# SQLx 共享游标关闭错误优先级恢复（2026-10-06）

完整workspace旧终态62698：exit101，249块3750PASS2FAIL，失败均来自task6_sqlx_canonical_matrix_test同一helper第163行。原完整Statement close合同在有效租约中要求精确Other("result set is closed")；失效租约优先ConnectionDiscarded。固定BASE SQLx type mapping原案例没有这个关闭后next补充断言；原值、标签、共享游标和零行descriptor合同全部保留。

只调整补充断言为精确关闭错误，保留同一真实SQLx SQLite当前视图直到connection.close，再精确断言ConnectionDiscarded。未修改生产行为、能力或原BASE断言。

新鲜目标测试19734终态exit0：7PASS0FAIL，日志task6-sqlx-close-error-priority-final.log。全仓3750/2为修复前源码，尚无修复后全仓结果。正式1283 sourcefloor1245/38未变，全部18顶层任务仍开放。
