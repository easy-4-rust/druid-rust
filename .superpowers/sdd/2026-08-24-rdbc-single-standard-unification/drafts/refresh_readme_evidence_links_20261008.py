from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


def section(text, start, end, replacement):
    assert text.count(start) == text.count(end) == 1
    left = text.index(start)
    right = text.index(end, left)
    return text[:left] + replacement.rstrip() + '\n\n' + text[right:]


updates = []
for filename in ['README.md', 'README.zh-CN.md']:
    path = ROOT / filename
    raw = path.read_bytes()
    newline = '\r\n' if b'\r\n' in raw else '\n'
    text = raw.decode('utf-8').replace('\r\n', '\n')
    english = filename == 'README.md'
    if english:
        status = '''### 1.3 Status evidence

Current acceptance is recorded in [verification status](docs/verification/rdbc-first/status.md),
not inferred from the commands listed in this README. P01/P02 have historical accepted
scopes; P04 Task6 remains FIX_REQUIRED and Task7–8/P05–P07 remain open. The
[2026-10-08 audit](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md)
separates source checks, actual targeted tests, historical workspace failures and missing
release evidence. No stable API, full database matrix or production certification is claimed.
'''
        examples = '''## 6. Executable Examples and Call Paths

The current tests use the single standard contract. These source links are entry points,
not an assertion that every target or feature has passed on the current revision.

### 6.1 Toasty and direct standard connections

- [Toasty canonical matrix](crates/druid-wrapper/tests/task6_toasty_canonical_matrix_test.rs)
- [SQLx canonical matrix](crates/druid-wrapper/tests/task6_sqlx_canonical_matrix_test.rs)
- [External Wrapper identity and borrow contracts](crates/druid-rdbc/tests/standard_wrapper_contract_test.rs)

### 6.2 Pooled connections, filters and prepared resources

- [Filter and statistics semantics](crates/druid/tests/standard_filter_stat_semantics_test.rs)
- [Prepared cache LRU regression](crates/druid/tests/task6_prepared_cache_lru_regression_test.rs)
- [Prepared close cleanup regression](crates/druid/tests/task6_prepared_close_cleanup_regression_test.rs)

### 6.3 External pools and migration evidence

- [SQLx bb8 pool](crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs)
- [SQLx deadpool pool](crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs)
- [Deleted-test migration ledger](docs/verification/rdbc-first/task6-deleted-test-migration.tsv)
- [Physical-to-standard migration](docs/rdbc/physical-to-standard-migration.md)

API signatures remain unstable. The migration ledger binds original behavior to current
test identities; a linked source file alone does not prove execution or equivalence.
'''
        navigation = '''## 11. Migration Roadmap and Phases

Execute the [RDBC-first master roadmap](docs/superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md)
and its existing child plans. The accepted [five-crate specification](docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md)
and [single-standard specification](docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md)
define the architecture and behavior requirements. Consult [verification status](docs/verification/rdbc-first/status.md)
for accepted scope, failures, prerequisites and open tasks. Module existence and source
checks cannot replace behavioral, real-integration or production-property evidence.

## 12. Documentation Set

| Document | Responsibility |
| :--- | :--- |
| [Architecture baseline](docs/druid-rust-Architecture.zh_CN.md) | Architecture history, invariants and ADRs; interpret historical snapshots with current evidence |
| [Accepted five-crate specification](docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md) | Product responsibilities and acceptance requirements |
| [Master roadmap](docs/superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md) | Plan dependencies and execution sequence |
| [Verification status](docs/verification/rdbc-first/status.md) | Actual evidence and open acceptance gates |
| [Current architecture/progress audit](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md) | Dated source, configuration and progress interpretation |

README provides navigation; specifications, plans and evidence remain their respective
sources of truth. Historical counts are not current completion percentages.
'''
        quality = '''## 13. Quality Gates

Fresh fmt/build/clippy/docs, complete C1/C2 runtime, compiler coverage, public API/SemVer,
feature combinations, MSRV, Java differential, real-database, network/security and supply-chain
gates are required by the plans. Their current status is recorded in the
[verification ledger](docs/verification/rdbc-first/status.md). Example commands below are
not recorded PASS results; default tests do not certify all features or release readiness.

'''
        end_quality = 'Recommended local commands:'
        replacements = [
            ('| Default Toasty integration | 🧪 Preview | `druid` |', '| Default Toasty integration | 🧪 Preview | `druid-wrapper` |'),
            ('| Explicit driver install and diagnostics | 🧪 Preview | `druid-admin` |', '| Explicit driver install and diagnostics | 🧪 Preview | `druid-wrapper` |'),
            ('| Complete Java semantics | 🚧 Partial | workspace | P0–P10 are not closed | object/semantic ledgers |', '| Complete Java semantics | 🚧 Partial | workspace | full semantic and production acceptance remains open | specifications and verification status |'),
        ]
        sections = [('### 1.3 Status evidence', '## 2. Features and Maturity', status),
                    ('## 6. Executable Examples and Call Paths', '### 6.4 80-database catalog and JDBC Agent', examples),
                    ('## 11. Migration Roadmap and Phases', '## 13. Quality Gates', navigation)]
    else:
        status = '''### 1.3 状态证据

当前验收以[验证状态](docs/verification/rdbc-first/status.md)为准，不能由 README 列出的
命令推定通过。P01/P02 已验收历史限定范围；P04 Task6 仍为 FIX_REQUIRED，Task7–8
与 P05–P07 仍开放。[2026-10-08 核验](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md)
明确区分源码检查、局部实际测试、历史全仓失败与缺失的发布证据。
当前不声明稳定 API、完整数据库矩阵或生产认证已经完成。
'''
        examples = '''## 6. 可执行示例与调用路径

当前测试使用唯一标准合同。下面的源码链接用于定位入口，不代表每个目标或 feature
已经在当前源码版本通过。

### 6.1 Toasty 与 direct 标准连接

- [Toasty canonical 矩阵](crates/druid-wrapper/tests/task6_toasty_canonical_matrix_test.rs)
- [SQLx canonical 矩阵](crates/druid-wrapper/tests/task6_sqlx_canonical_matrix_test.rs)
- [外部 Wrapper 身份与借用合同](crates/druid-rdbc/tests/standard_wrapper_contract_test.rs)

### 6.2 池化连接、Filter 与 prepared 资源

- [Filter 与统计语义](crates/druid/tests/standard_filter_stat_semantics_test.rs)
- [Prepared cache LRU 回归](crates/druid/tests/task6_prepared_cache_lru_regression_test.rs)
- [Prepared close 清理回归](crates/druid/tests/task6_prepared_close_cleanup_regression_test.rs)

### 6.3 外部池与迁移证据

- [SQLx bb8 pool](crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs)
- [SQLx deadpool pool](crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs)
- [删除测试迁移账本](docs/verification/rdbc-first/task6-deleted-test-migration.tsv)
- [Physical-to-standard 迁移](docs/rdbc/physical-to-standard-migration.md)

API 签名尚未稳定。迁移账本将原始行为绑定到当前测试身份；源码链接本身不能证明
实际执行或行为等价。
'''
        navigation = '''## 11. 迁移路线与阶段

执行已有 [RDBC-first 主路线](docs/superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md)
及其子计划。已批准的[五 crate 规格](docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md)
与[唯一标准规格](docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md)
定义架构和行为要求。[验证状态](docs/verification/rdbc-first/status.md)记录已接受范围、
失败、前置条件和开放任务。模块存在或源码检查不能替代行为、真实集成和生产属性验收。

## 12. 文档集合

| 文档 | 责任 |
| :--- | :--- |
| [架构基线](docs/druid-rust-Architecture.zh_CN.md) | 架构历史、不变量与 ADR；历史快照需结合当前证据解读 |
| [五 crate 正式规格](docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md) | 产品职责与验收要求 |
| [主路线](docs/superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md) | 计划依赖与执行顺序 |
| [验证状态](docs/verification/rdbc-first/status.md) | 实际证据与开放门禁 |
| [当前架构进度核验](docs/verification/rdbc-first/architecture-progress-audit-2026-10-08.md) | 有日期的源码、配置与进度判断 |

README 提供导航；规格、计划与验证证据分别保留原有事实源。历史计数不是当前完成率。
'''
        quality = '''## 13. 质量门禁

计划要求新鲜 fmt/build/clippy/docs、完整 C1/C2 运行、编译覆盖、公开 API/SemVer、
feature 组合、MSRV、Java 差分、真实数据库、网络安全及供应链验证。
当前状态以[验证账本](docs/verification/rdbc-first/status.md)为准。下面的命令是用法，
不是 PASS 记录；默认测试不能认证全部 features 或发布就绪。

'''
        end_quality = '推荐本地命令：'
        replacements = [
            ('| Toasty 默认集成 | 🧪 预览 | `druid` |', '| Toasty 默认集成 | 🧪 预览 | `druid-wrapper` |'),
            ('| 驱动显式安装与诊断 | 🧪 预览 | `druid-admin` |', '| 驱动显式安装与诊断 | 🧪 预览 | `druid-wrapper` |'),
            ('| Java 全对象语义 | 🚧 部分 | workspace | P0–P10 未全部关闭 | 对象/语义总账 |', '| Java 全对象语义 | 🚧 部分 | workspace | 完整语义与生产验收仍开放 | 正式规格与验证状态 |'),
        ]
        sections = [('### 1.3 状态证据', '## 2. 功能与成熟度', status),
                    ('## 6. 可执行示例与调用路径', '### 6.4 80 数据库目录与 JDBC Agent', examples),
                    ('## 11. 迁移路线与阶段', '## 13. 质量门禁', navigation)]
    for start, end, replacement in sections:
        text = section(text, start, end, replacement)
    heading = '## 13. Quality Gates' if english else '## 13. 质量门禁'
    text = section(text, heading, end_quality, quality)
    for old, new in replacements:
        assert text.count(old) == 1, (filename, old)
        text = text.replace(old, new, 1)
    assert text.count('(docs/README.md)') == 1
    text = text.replace('(docs/README.md)', '(docs/superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md)', 1)
    updates.append((path, text.replace('\n', newline).encode('utf-8')))

for path, payload in updates:
    path.write_bytes(payload)
    print('UPDATED', path.name)
