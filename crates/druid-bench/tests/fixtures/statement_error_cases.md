# Frozen Statement error observations

`statement_error_cases.tsv` contains the 37 original calls from Java Druid
1.2.28 `DruidPooledStatementTest`, commit
`33824c3dec1612711f9bb4e409319bcab2e4cd0e`. The original test source SHA-256 is
`4ee9bd8bd897e9f7aae605ef5bc18afed0766a0fa2a5ff27b5c3d567b0dab40e`.

The corpus carries explicit method names, argument shapes and values, including
null cursor names, direction 0, timeout -1 and empty generated-key selector
arrays. The Java helper executes those fields, rather than deriving an expected
exception from the case ID. The final `java_call` column preserves the original
source expression for review. `-` denotes null for text inputs; empty text remains
an empty field. The fixed corpus SHA-256 is
`2fa81686348ffe8abe951181735fb1c3916d9df0038267fb87177d9ea5e1f04d`.

The helper recompiles with the original test class and uses its unchanged setup,
private ErrorFilter and teardown. It rejects a runtime version other than 1.2.28,
corpus digest drift, duplicate cases, or a denominator differing from the original
37 reflected test methods. The production classpath must come from the frozen
1.2.28 build; this helper does not rebuild the entire Java production library.

Example PowerShell invocation from the repository root, using the same explicit
environment variables as the exception-sorter oracle:

```powershell
$taskRoot = $env:DRUID_JAVA_ORACLE_ROOT
$taskSource = Join-Path $taskRoot 'core/src/test/java/com/alibaba/druid/bvt/pool/DruidPooledStatementTest.java'
if ((Get-FileHash $taskSource).Hash -ne '4EE9BD8BD897E9F7AAE605EF5BC18AFED0766A0FA2A5FF27B5C3D567B0DAB40E') { throw 'Frozen source drift' }
$taskEvidence = Join-Path $env:DRUID_JAVA_ORACLE_WORK_DIR ('statement-errors-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $taskEvidence | Out-Null
& "$env:DRUID_JAVA_ORACLE_JAVA_HOME/bin/javac.exe" -encoding UTF-8 -cp $env:DRUID_JAVA_ORACLE_CLASSPATH -d $taskEvidence $taskSource crates/druid-bench/tests/fixtures/StatementErrorCasesOracle.java
if ($LASTEXITCODE -ne 0) { throw 'Oracle compilation failed' }
& "$env:DRUID_JAVA_ORACLE_JAVA_HOME/bin/java.exe" "-Djava.io.tmpdir=$taskEvidence" -cp "$taskEvidence;$env:DRUID_JAVA_ORACLE_CLASSPATH" StatementErrorCasesOracle crates/druid-bench/tests/fixtures/statement_error_cases.tsv "$taskEvidence/java-results.tsv"
if ($LASTEXITCODE -ne 0) { throw 'Oracle execution failed' }
```

The 2026-10-20 JDK 17.0.12 run observed 35 plain SQL exceptions with one error
each, and two unsupported close-on-completion operations with no error-count
increment. Every case returned to active/idle 0/1 and reused the same physical
holder. Error state and message columns contain lossless UTF-16 hex; `-` means
null and an empty field means an empty string. Original logs remain separate
from the structured UTF-8 result file.

These are Java observations only. A live Rust comparison has not yet been added
for this corpus. The two close-on-completion cases correspond to the user's
approved standard-behavior difference; a future comparator must retain them as
explicit allowed differences, rather than remove them or report exact matches.
