from pathlib import Path
import subprocess, os, json, hashlib
work = Path(r'D:\codex-build\druid-sorter-methods-20261101')
repo = Path(r'E:\workspaces\workspace-github-easy-4-rust\druid-rust')
java = Path(r'E:\workspaces\workspace-github\druid')
classes = work/'current-source-classes'
classes.mkdir(exist_ok=True)
jdk = Path(os.environ['DRUID_JAVA_ORACLE_JAVA_HOME'])/'bin'
m2 = Path(r'C:\Users\hnxyh\.m2\repository')
jars = [m2/'com/alibaba/druid/1.2.28/druid-1.2.28.jar', m2/'org/junit/platform/junit-platform-console-standalone/1.11.4/junit-platform-console-standalone-1.11.4.jar', m2/'org/postgresql/postgresql/42.7.8/postgresql-42.7.8.jar', m2/'mysql/mysql-connector-java/8.0.28/mysql-connector-java-8.0.28.jar']
for jar in jars:
    assert jar.is_file(), jar
cp = os.pathsep.join(map(str, jars))
sources = [java/line.split('\t')[0] for line in (repo/'crates/druid-bench/tests/fixtures/exception_sorter_java_sources.tsv').read_text(encoding='utf-8').splitlines() if line and not line.startswith('#')]
tests = sorted((java/'core/src/test/java/com/alibaba/druid/bvt/pool/vendor').glob('*ExceptionSorterTest*.java'))
sources += [java/'core/src/main/java/com/alibaba/druid/pool/vendor/MockExceptionSorter.java', java/'core/src/test/java/com/alibaba/druid/PoolTestCase.java', *tests, java/'core/src/test/java/com/alibaba/druid/bvt/pool/ExceptionSorterTest.java', repo/'crates/druid-bench/tests/fixtures/MockSorterCaseOracle.java', work/'SourceSorterTests.java']
commands = []
def run(name, args):
    command = list(map(str, args))
    commands.append({'name': name, 'argv': command})
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    (work/'logs'/f'{name}.stdout').write_bytes(result.stdout)
    (work/'logs'/f'{name}.stderr').write_bytes(result.stderr)
    (work/'logs'/f'{name}.exit').write_text(str(result.returncode), encoding='utf-8')
    print(name, result.returncode)
    if result.returncode:
        print(result.stderr.decode('utf-8', errors='replace'))
    result.check_returncode()
run('current-source-javac', [jdk/'javac.exe', '-J-Dfile.encoding=UTF-8', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *sources])
base = [jdk/'java.exe', '-Dfile.encoding=UTF-8', '-Djava.io.tmpdir='+os.environ['TEMP'], '-cp', str(classes)+os.pathsep+cp]
run('current-source-tests', [*base, 'SourceSorterTests', *[p.stem for p in tests]])
run('current-mock-java', [*base, 'MockSorterCaseOracle', repo/'crates/druid-bench/tests/fixtures/mock_sorter_cases.tsv', work/'current-mock-results.tsv'])
frozen = sorted((work/'oracle').glob('mock-sorter-live-*/rust-results.tsv'), key=lambda p:p.stat().st_mtime)[-1]
assert (work/'current-mock-results.tsv').read_bytes() == frozen.read_bytes(), 'current Mock observation differs from Rust'
(work/'current-java-commands.json').write_text(json.dumps(commands, indent=2), encoding='utf-8')
(work/'current-java-inputs.json').write_text(json.dumps([{'path':str(p), 'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sources+jars], indent=2), encoding='utf-8')
print('current_mock_vs_rust MATCH; frozen_rust_results', frozen)
