"""最终语义门禁的当前公开形态、来源冻结及原始运行证据执行。"""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

from verify_jdbc42_semantic_inventory import VerificationError


def verify_api_shapes(rows: list[list[str]], public_api: str) -> None:
    """验证新鲜 cargo-public-api 声明种类；单纯 import 成功不能认证 enum/module。"""
    declarations = {}
    aliases = {}
    unit_variants = set()
    pattern = r"^(?:#\[non_exhaustive\] )?pub (trait|struct|enum|mod|type) (druid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+)"
    for line in public_api.splitlines():
        match = re.match(pattern, line)
        if not match:
            variant = re.fullmatch(r"pub (druid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+)", line)
            if variant:
                unit_variants.add(variant[1])
            continue
        kind, target = match.groups()
        if target in declarations and declarations[target] != kind:
            raise VerificationError(f"public API has ambiguous declaration: {target}")
        declarations[target] = kind
        if kind == "type":
            alias = re.fullmatch(re.escape(match.group(0)) + r" = (druid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+);?", line)
            if alias:
                aliases[target] = alias[1]
    # cargo-public-api 的单元成员没有 enum 前缀；必须由直接父枚举声明佐证。
    # 先收集完整声明，避免输出顺序影响结果，也不从父枚举推断缺失成员。
    for target in unit_variants:
        if declarations.get(target.rsplit("::", 1)[0]) == "enum":
            if target in declarations and declarations[target] != "enum_variant":
                raise VerificationError(f"public API has ambiguous declaration: {target}")
            declarations[target] = "enum_variant"
    for row in rows:
        shape, disposition, target = row[3:6]
        if shape == "EXCLUDED_RUNTIME":
            if disposition != "JAVA_RUNTIME_ONLY" or target != "host::process_security_policy":
                raise VerificationError(f"invalid final host exclusion: {row[0]}")
            continue
        if not re.fullmatch(r"druid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+", target):
            raise VerificationError(f"invalid final canonical target: {target}")
        seen = set()
        resolved = target
        while resolved in aliases:
            if resolved in seen:
                raise VerificationError(f"public API alias cycle: {target}")
            seen.add(resolved)
            resolved = aliases[resolved]
        actual = declarations.get(resolved)
        expected = {"TRAIT": {"trait"}, "CALLBACK": {"trait"}, "DTO": {"struct", "enum"},
                    "ENUM_KIND": {"enum", "enum_variant"}, "MODULE": {"mod"}}.get(shape, set())
        if actual not in expected:
            raise VerificationError(f"public API kind mismatch: {target} expected {shape}, actual {actual}")


def require_closed_druid_evidence(summary) -> None:
    """拒绝仍保留兼容或资源待办的旧阶段验证结果。"""
    if summary.task6_pending_rows:
        raise VerificationError(f"final Druid Task6 evidence pending: {summary.task6_pending_rows}")
    if summary.task7_pending_rows:
        raise VerificationError(f"final Druid Task7 resource evidence pending: {summary.task7_pending_rows}")


def input_hashes(root: Path) -> dict[str, str]:
    """冻结实际生产、测试、验证工具和根级证据清单，不将运行日志作为输入。"""
    paths = {root / "Cargo.toml", root / "Cargo.lock", root / "rust-toolchain.toml"}
    for folder in ("crates", "vendor", "scripts", "docs", "api", ".cargo"):
        for path in (root / folder).rglob("*"):
            if path.is_file() and not {"__pycache__", "target"}.intersection(path.relative_to(root).parts):
                paths.add(path)
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(paths) if p.is_file()}


def verify_runtime(root: Path, rows: list[list[str]]) -> None:
    """执行当前编译 API 和完整原门禁；不安装工具、不回退历史阶段或跳过运行。"""
    from verify_no_physical_standard import verify_repository as no_physical
    from verify_rdbc_single_standard_purity import verify_tree
    from verify_wrapper_standard_impls import verify_repository as wrapper
    from verify_druid_single_standard import verify_repository as druid

    root = root.resolve()
    before = input_hashes(root)
    parent = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification/final-semantic-runs"
    parent.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="run-", dir=parent))
    summary = {"status": "RUNNING", "scope": "FINAL_SEMANTIC_EVIDENCE_NOT_RELEASE_CERTIFICATION",
               "input_sha256": before, "completed_gates": []}
    destination = output / "summary.json"

    def save():
        destination.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    save()
    try:
        # 新鲜工具输出取代可手改或过期的 api/*.txt，工具缺失直接失败。
        command = ["cargo", "public-api", "-p", "druid-rdbc", "-sss", "--all-features"]
        environment = os.environ.copy()
        environment["RUSTUP_AUTO_INSTALL"] = "0"
        environment["CARGO_NET_OFFLINE"] = "true"
        summary["public_api_command"] = command
        with (output / "public-api.txt").open("wb") as stdout, (output / "public-api.stderr.log").open("wb") as stderr:
            result = subprocess.run(command, cwd=root, env=environment, stdout=stdout, stderr=stderr)
        summary["public_api_exit_code"] = result.returncode
        summary["public_api_sha256"] = hashlib.sha256((output / "public-api.txt").read_bytes()).hexdigest()
        save()
        if result.returncode != 0:
            raise VerificationError(f"fresh public API compiler gate failed: exit {result.returncode}; {output}")
        verify_api_shapes(rows, (output / "public-api.txt").read_text(encoding="utf-8"))
        summary["completed_gates"].append("CURRENT_PUBLIC_API_KINDS")
        save()
        verify_tree(root / "crates/druid-rdbc/src", phase="final")
        summary["completed_gates"].append("FINAL_STANDARD_PURITY")
        save()
        # 直接调用原完整验证器；其编译、运行及原超时规则保持不变。
        no_physical(root)
        summary["completed_gates"].append("NO_PHYSICAL_WITH_ORIGINAL_DELETED_TEST_RUNTIME")
        save()
        wrapper(root)
        summary["completed_gates"].append("WRAPPER_WITH_COMPILER_COVERAGE_AND_RUNTIME")
        save()
        require_closed_druid_evidence(druid(root))
        summary["completed_gates"].append("DRUID_WITH_NO_PENDING_COMPATIBILITY_OR_RESOURCE_ROWS")
        save()
        if input_hashes(root) != before:
            raise VerificationError("final semantic evidence inputs changed during verification")
        summary["status"] = "PASS_FINAL_SEMANTIC_EVIDENCE"
        save()
    except (OSError, UnicodeError, RuntimeError, subprocess.SubprocessError) as error:
        summary.update(status="FAIL_FINAL_SEMANTIC_EVIDENCE", error=str(error))
        save()
        raise VerificationError(f"final runtime evidence failed: {error}; receipt {destination}") from error
