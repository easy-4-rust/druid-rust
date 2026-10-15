"""Prove the frozen resource-capability consumers use the standard operations type."""

from collections import Counter
import csv
from pathlib import Path
import re
import subprocess


TARGET = "druid_rdbc::resource::ResourceOperations"
OLD = "druid_rdbc::resource::RdbcResourceCapabilities"
PENDING = "TASK7_RESOURCE_OPERATIONS_PENDING"
CLOSURE_HEADER = ("source_path", "occurrence_kind", "symbol", "old_path", "accepted_target")
ROOT = Path(__file__).resolve().parents[1]
HISTORY = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
LEDGER = Path("docs/verification/rdbc-first/task7-resource-operations-closure.tsv")


def verify_closure_rows(historical: list[dict[str, str]], closure: list[tuple[str, ...]]) -> int:
    """Require an exact one-to-one match with all 108 original occurrence identities."""
    original = [row for row in historical if row["old_path"] == OLD and row["state"] == PENDING]
    if len(original) != 108:
        raise ValueError(f"expected 108 historical resource capability rows, found {len(original)}")
    expected = Counter(tuple(row[key] for key in CLOSURE_HEADER[:-1]) + (TARGET,) for row in original)
    actual = Counter(closure)
    if any(len(row) != len(CLOSURE_HEADER) or row[-1] != TARGET for row in closure):
        raise ValueError("resource closure has an invalid standard target")
    if actual != expected:
        raise ValueError(f"resource closure identity drift: missing={sum((expected - actual).values())} extra={sum((actual - expected).values())}")
    if any(count != 1 for count in expected.values()):
        raise ValueError("historical resource capability identity is duplicated")
    return len(original)


def verify_source_tree(root: Path) -> None:
    """Reject old production names and require the canonical standard definition/export."""
    resource = root / "crates/druid-rdbc/src/resource"
    definition = resource / "resource_operations.rs"
    module = resource / "mod.rs"
    if not definition.is_file() or not module.is_file():
        raise ValueError("standard ResourceOperations module is missing")
    if (resource / "rdbc_resource_capabilities.rs").exists():
        raise ValueError("retired resource capabilities module remains")
    if not re.search(r"\bpub\s+struct\s+ResourceOperations\b", definition.read_text(encoding="utf-8")):
        raise ValueError("standard ResourceOperations declaration is missing")
    module_source = module.read_text(encoding="utf-8")
    if "mod resource_operations;" not in module_source or "pub use resource_operations::ResourceOperations;" not in module_source:
        raise ValueError("standard ResourceOperations export is missing")
    for source_root in (root / "crates").glob("*/src"):
        for path in source_root.rglob("*.rs"):
            if re.search(r"\bRdbcResourceCapabilities\b", path.read_text(encoding="utf-8")):
                raise ValueError(f"retired resource capability name remains in {path.relative_to(root)}")


def read_closure(path: Path) -> list[tuple[str, ...]]:
    """Read the canonical LF-only closure ledger with an exact schema."""
    payload = path.read_bytes()
    if not payload.endswith(b"\n") or b"\r" in payload or payload.endswith(b"\n\n"):
        raise ValueError("resource closure ledger is not canonical LF TSV")
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.reader(stream, delimiter="\t")
        if tuple(next(reader, ())) != CLOSURE_HEADER:
            raise ValueError("resource closure ledger header drift")
        rows = [tuple(row) for row in reader]
    if rows != sorted(rows):
        raise ValueError("resource closure ledger is not sorted")
    return rows


def verify_repository(root: Path = ROOT) -> int:
    """Verify exact history, production surface, and live resource behavior."""
    with (root / HISTORY).open(encoding="utf-8", newline="") as stream:
        historical = list(csv.DictReader(stream, delimiter="\t"))
    closed = verify_closure_rows(historical, read_closure(root / LEDGER))
    verify_source_tree(root)
    completed = subprocess.run(
        ["cargo", "test", "-p", "druid-rdbc", "--locked", "--offline", "--test", "resource_standard_contract_test"],
        cwd=root, check=False, capture_output=True, text=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"standard resource runtime contract failed: {completed.stderr[-2000:]}")
    return closed


if __name__ == "__main__":
    print(f"TASK7_RESOURCE_OPERATIONS_CLOSED={verify_repository()}")
