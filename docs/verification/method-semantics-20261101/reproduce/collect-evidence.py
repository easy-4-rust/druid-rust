from pathlib import Path
import json, csv, re, hashlib, subprocess, shutil
work = Path(r'D:\codex-build\druid-sorter-methods-20261101')
repo = Path(r'E:\workspaces\workspace-github-easy-4-rust\druid-rust')
java = Path(r'E:\workspaces\workspace-github\druid')
dest = repo/'docs/verification/method-semantics-20261101'
dest.mkdir(parents=True, exist_ok=True)
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def write_json(p, value): p.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
def tsv(name, rows):
    with (dest/name).open('w', encoding='utf-8', newline='') as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]), delimiter='\t', lineterminator='\n'); w.writeheader(); w.writerows(rows)
baseline = json.loads((work/'baseline.json').read_text(encoding='utf-8'))
ending = {p: digest(Path(p)) if Path(p).is_file() else None for p in baseline['input_sha256']}
write_json(dest/'baseline.json', baseline)
write_json(dest/'end-state.json', {'rust_head':subprocess.check_output(['git','-C',str(repo),'rev-parse','HEAD'],text=True).strip(), 'java_head':subprocess.check_output(['git','-C',str(java),'rev-parse','HEAD'],text=True).strip(), 'inputs':[{'path':p,'before_sha256':h,'after_sha256':ending[p],'changed':h!=ending[p]} for p,h in baseline['input_sha256'].items()]})
for name in ['logs','reproduce']:
    (dest/name).mkdir(exist_ok=True)
for p in (work/'logs').iterdir():
    if p.is_file(): shutil.copyfile(p, dest/'logs'/p.name)
for name in ['environment.ps1','prepare-source-validation.py','run-current-java.py','SourceSorterTests.java','SorterPropertyProbe.java','generate-property-cases.py']:
    shutil.copyfile(work/name, dest/'reproduce'/name)
for name in ['current-java-commands.json','current-java-inputs.json','source-tests-manifest.json','current-mock-results.tsv']:
    shutil.copyfile(work/name, dest/name)
# 保存每次对照的文本证据，不复制 class/build 产物；保留 RED 与 GREEN。
runs=[]
for run in sorted((work/'oracle').iterdir()):
    if not (run/'comparison.tsv').is_file(): continue
    target=dest/'oracle'/run.name; target.mkdir(parents=True, exist_ok=True)
    for p in run.iterdir():
        if p.is_file() and p.suffix in {'.tsv','.stdout','.stderr','.exit'}:
            shutil.copyfile(p,target/p.name)
    records=list(csv.DictReader((run/'comparison.tsv').read_text(encoding='utf-8').splitlines(),delimiter='\t'))
    counts={}
    for row in records: counts[row['status']]=counts.get(row['status'],0)+1
    runs.append({'run':run.name,'rows':len(records),'statuses':counts})
write_json(dest/'oracle-runs.json',runs)
cov=json.loads((work/'coverage-selected.json').read_text(encoding='utf-8'))['data'][0]
selected=[f for f in cov['files'] if 'exception_sorter' in f['filename']]
coverage_rows=[]
for f in selected:
    row={'path':str(Path(f['filename']).relative_to(repo)).replace('\\','/')}
    for key in ('regions','functions','lines'):
        s=f['summary'][key];row[key+'_covered']=s['covered'];row[key+'_count']=s['count'];row[key+'_percent']=round(s['percent'],4)
    coverage_rows.append(row)
tsv('sorter-coverage.tsv',coverage_rows)
write_json(dest/'coverage-summary.json',{'totals':cov['totals'],'file_count':len(cov['files']),'raw_selected_json':str(work/'coverage-selected.json'),'raw_selected_sha256':digest(work/'coverage-selected.json'),'raw_unselected_json':str(work/'coverage.json'),'raw_unselected_sha256':digest(work/'coverage.json'),'branch_coverage':'NOT_COLLECTED','production_exclusions':[]})
names={'ExceptionSorter':'exception_sorter','AbstractOracleExceptionSorter':'abstract_oracle_exception_sorter','DB2ExceptionSorter':'db2_exception_sorter','InformixExceptionSorter':'informix_exception_sorter','MockExceptionSorter':'mock_exception_sorter','MySqlExceptionSorter':'my_sql_exception_sorter','NullExceptionSorter':'null_exception_sorter','OceanBaseOracleExceptionSorter':'ocean_base_oracle_exception_sorter','OracleExceptionSorter':'oracle_exception_sorter','PGExceptionSorter':'pg_exception_sorter','PhoenixExceptionSorter':'phoenix_exception_sorter','SybaseExceptionSorter':'sybase_exception_sorter'}
contracts={
'DB2ExceptionSorter':'recoverable 优先；SQLState 08 前缀；8 个负错误码；其他 false',
'InformixExceptionSorter':'recoverable 优先；17 个负错误码；其他 false；不以 SQLState 扩大范围',
'MockExceptionSorter':'MockConnectionClosedException instanceof/继承身份；不以消息或错误码替代',
'MySqlExceptionSorter':'recoverable/08；19 个错误码；[-9000,-8000]；异常类后缀；streaming 前后缀；message 条件优先级；最多 5 层 cause',
'NullExceptionSorter':'恒 false，包括 recoverable 异常',
'OceanBaseOracleExceptionSorter':'recoverable/08；wrapping abs；49 个错误码及 TNS 区间；20000..20999 消息排除；5 个附加消息；自定义集合',
'OracleExceptionSorter':'recoverable；wrapping abs；49 个错误码及 TNS 区间；20000..20999 消息排除；自定义集合；不增加 SQLState 08 条件',
'PGExceptionSorter':'recoverable 优先；null SQLState false；08 前缀 true；其他 false',
'PhoenixExceptionSorter':'区分大小写的 Connection is null or closed 子串；命中返回 true',
'SybaseExceptionSorter':'recoverable 优先；null message false；大写后的 JZ0C0/JZ0C1 子串'}
rows=[]
for name,stem in names.items():
    path=java/'core/src/main/java/com/alibaba/druid/pool'/('ExceptionSorter.java' if name=='ExceptionSorter' else 'vendor/'+name+'.java')
    rust_path=repo/'crates/druid/src/core'/(stem+'.rs')
    for number,line in enumerate(path.read_text(encoding='utf-8').splitlines(),1):
        if not re.match(r'\s*(public |protected |boolean |void )', line): continue
        match=re.match(r'\s*(?:(?:public|protected)\s+(?:static\s+)?(?:final\s+)?)?(?:(?:boolean|void|Set<Integer>|\w+ExceptionSorter)\s+)?(\w+)\([^;{}]*\)\s*[;{]',line)
        if not match: continue
        method=match[1]
        if method not in {'isExceptionFatal','configFromProperties','getInstance','getFatalErrorCodes','setFatalErrorCodes',name}: continue
        rust_method={'isExceptionFatal':'is_exception_fatal','configFromProperties':'config_from_properties','getInstance':'get_instance','getFatalErrorCodes':'fatal_error_codes','setFatalErrorCodes':'set_fatal_error_codes'}.get(method,'new')
        locations=[str(i) for i,l in enumerate(rust_path.read_text(encoding='utf-8').splitlines(),1) if re.search(r'\bfn '+rust_method+r'\b',l)]
        evidence='V2: exception_sorter_semantics_test'; gap='没有以本行证据授予对象整体完成状态'
        category='已实现但缺验证（本批补证）'
        if name=='ExceptionSorter':
            contract='接口输入保留 SQLException 类型、码、SQLState、消息、cause；Properties 可空' if method=='isExceptionFatal' else '可空配置入口，具体行为由各 sorter 实现'
            evidence='源码合同审阅；17 项 Rust 池/分类回归';gap='Rust 非空引用/类型映射不是 JVM null 对象或反射序列化等价证明'
        elif method=='isExceptionFatal':
            contract=contracts[name]
            evidence+='; V4: 原576差分' if name!='MockExceptionSorter' else '; V4: Mock1例字段差分（Java池依赖1.2.28 jar）'
            if name in ('OracleExceptionSorter','OceanBaseOracleExceptionSorter'):
                evidence+='; V4: 补充118差分';gap='各有1条既有 null-message approved difference；默认 Locale 与其他 JDK Unicode 版本未验证'
            elif name=='PhoenixExceptionSorter':
                category='确实缺失（日志副作用待补）';gap='Java命中时LOG.error未迁移；另有1条既有null-message批准差异'
            elif name in ('MySqlExceptionSorter','SybaseExceptionSorter'): gap='默认 Locale 行为未验证；cause 表示已扁平化，未证明所有驱动构建路径'
        elif method=='configFromProperties':
            if name=='AbstractOracleExceptionSorter':
                category='确实缺失（数字解析已修复；日志待补）';contract='null/缺键不变；逗号分割、不trim；Java整数解析；去重追加'
                evidence='RED:2 Rust失败/84差分不符；GREEN:2 Rust通过/118 MATCH';gap='非法项LOG.error副作用仍未迁移；Properties defaults/non-string值不在当前Rust映射中'
            else:
                category='已实现但文档滞后（空方法为源语义）';contract='Java 源方法为空；Rust 保留无操作，不能归为 STUB';gap='空实现已核对；不是遗漏需重写的业务功能'
        elif method=='getInstance':
            contract='进程级静态共享实例';evidence='V2: singleton identity 断言；源码静态实例核对';gap='Rust零大小类型地址比较不能证明Java对象身份的所有外部用法'
        elif method in ('getFatalErrorCodes','setFatalErrorCodes'):
            contract='读取当前集合' if method=='getFatalErrorCodes' else '整个集合替换，不追加'
            evidence='V2: exception_sorter_semantics_test + property_contract exact set';gap='Java可变Set别名/null setter与Rust借用/所有权差异尚未逐项定性；不称全API形状一致'
        else:
            contract='构造时读取 System.getProperties，Rust 使用同名进程环境变量' if name!='SybaseExceptionSorter' else '构造时调用空configFromProperties，最终无状态'
            evidence='源码审阅；默认构造路径本地已执行';gap='Oracle环境变量存在/非法Unicode进程边界尚缺隔离子进程验证' if name!='SybaseExceptionSorter' else '无状态构造等价；不迁移Java Serializable标记为业务逻辑'
        rows.append({'java_object':name,'java_method':line.strip().rstrip('{;').strip(),'java_path':str(path.relative_to(java)).replace('\\','/'),'java_line':number,'java_sha256':digest(path),'rust_path':str(rust_path.relative_to(repo)).replace('\\','/'),'rust_method':rust_method,'rust_line':','.join(locations) or 'Default/unit struct','reviewed_contract':contract,'classification':category,'evidence':evidence,'remaining_boundary':gap})
assert len(rows)==30, len(rows)
tsv('methods.tsv',rows)
source_rows=[]
for line in (work/'logs/current-source-tests.stdout').read_text(encoding='utf-8').splitlines():
    if not line.startswith('PASS\t'): continue
    cls,method=line.split('\t')[1].split('#');p=java/'core/src/test/java/com/alibaba/druid/bvt/pool/vendor'/(cls+'.java')
    source_rows.append({'source_test':str(p.relative_to(java)).replace('\\','/'),'source_sha256':digest(p),'method':method,'execution':'PASS original method + PoolTestCase lifecycle','level':'SOURCE_TEST_EXECUTED (not one-to-one V4 claim)'})
assert len(source_rows)==18
tsv('source-test-methods.tsv',source_rows)
print('method_rows',len(rows),'coverage_totals',cov['totals']['lines'],'oracle_runs',runs)
print('concurrent_changes',[(p,baseline['input_sha256'][p],h) for p,h in ending.items() if h!=baseline['input_sha256'][p]])
