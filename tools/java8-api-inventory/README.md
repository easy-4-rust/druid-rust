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
