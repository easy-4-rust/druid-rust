# JDBC Java 8 API inventory

`JdbcApiInventoryDoclet.java` extracts public/protected API declarations directly from Java source. It records types, constructors, constants, every overload, default methods, and inherited declarations with their declaring source file and line. The extractor does not read Rust symbols or use the Rust inventory to decide which Java members exist.

The pinned source oracle is OpenJDK `jdk8u502-b07` at commit `7373384725418fa645fb4508ec05a869bc7fc3e7`; the generator verifies the archive SHA-256 before reading it. The checked-in TSV is plain UTF-8 text and contains no copied JDK source or binary archive.

Run the fixture contract with an installed JDK 8:

```powershell
python -m unittest discover -s tools/java8-api-inventory -p 'test_*.py' -v
```

Regenerate the full inventory with `JAVA8_HOME` pointing to a JDK 8 installation. The script downloads the exact pinned source archive to a temporary directory when `--archive` is omitted, extracts only the 69 `java.sql` and `javax.sql` source files, and deletes the archive and extracted sources at exit:

```powershell
python scripts/generate_java8_api_method_inventory.py
```

For an already cached source archive, pass `--archive <path>`. To write elsewhere, pass `--output <path>`. The output is sorted and includes the pinned source commit in its header.

## Method evidence ledger

`docs/verification/rdbc-first/java8-method-evidence.tsv` uses the full frozen inventory as its denominator. Each evaluated link records the Rust source path and SHA-256, the linked test source SHA-256, and an inventory SHA-256/source commit in the ledger header. Re-run `python scripts/verify_java8_method_evidence.py` after regenerating the inventory or changing a linked source file. A stale inventory header or test/Rust source hash fails closed; review and refresh the affected evidence instead of copying a new hash without rerunning its test.

Layer values distinguish `PASS`, `FAIL`, `NOT_RUN`, `NOT_APPLICABLE`, and `UNASSESSED`. Capability values distinguish `SUPPORTED`, `PARTIAL`, `UNSUPPORTED`, `NOT_RUN`, and `UNASSESSED`. `NOT_RUN` needs a reason; `SUPPORTED` needs a method mapping, positive test reference, and a passing layer; `UNSUPPORTED` needs a negative test reference and exact observable error text. A SQLState alone does not prove the precise failure. `PARTIAL` records only the linked layers and never implies that unassessed layers passed.
