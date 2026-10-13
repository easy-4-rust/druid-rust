"""用真实rustc逐行验证JDBC总账公开入口；不认证行为或完整Rust形态。"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

from verify_jdbc42_semantic_inventory import (
    DEFAULT_SEMANTIC, RUST_SHAPES, VerificationError, parse_semantic_payload,
)


def digest(path: Path) -> str:
    """计算实际文件字节摘要，绑定本次编译输入。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def probe_source(shape: str, target: str) -> tuple[str, str]:
    """生成编译探针；trait采用bound，其他形态只声明有限认证范围。"""
    if shape not in RUST_SHAPES:
        raise VerificationError(f"unsupported rust_shape: {shape}")
    if shape == "EXCLUDED_RUNTIME":
        if target != "host::process_security_policy":
            raise VerificationError(f"invalid excluded runtime target: {target}")
        return "", "host policy excluded; no Rust certification"
    if not re.fullmatch(r"druid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+", target):
        raise VerificationError(f"invalid canonical Rust entry: {target}")
    prefix = "#![forbid(unsafe_code)]\n"
    if shape in {"TRAIT", "CALLBACK"}:
        return prefix + f"pub fn probe<T: {target} + ?Sized>() {{}}\n", "public trait bound resolves"
    if shape == "DTO":
        return prefix + f"pub fn probe(_value: {target}) {{}}\n", "public type resolves; DTO structure not certified"
    if shape == "ENUM_KIND" and len(target.split("::")) >= 4:
        # use 语句无法导入关联常量；类型成员路径（枚举变体与关联常量）用值位置证明可解析。
        return (
            prefix + f"pub fn probe() {{ let _ = {target}; }}\n",
            "public member value resolves; variants and associated consts",
        )
    return prefix + f"pub use {target} as Probe;\n", "public import resolution only"


def verify(arguments: argparse.Namespace) -> dict:
    """保留逐项编译日志；失败后继续，最终输入漂移则拒绝出具成功收据。"""
    semantic = arguments.semantic.resolve()
    artifact = arguments.artifact.resolve()
    dependency = arguments.dependency_dir.resolve()
    output = arguments.output_dir.resolve()
    if output.exists():
        raise VerificationError(f"output directory already exists: {output}")
    for path in (semantic, artifact, arguments.rustc):
        if not path.is_file():
            raise VerificationError(f"input file does not exist: {path}")
    if not dependency.is_dir():
        raise VerificationError(f"dependency directory does not exist: {dependency}")
    # 摘要必须绑定实际解析的字节，不能在解析后重读已被替换的总账。
    semantic_payload = semantic.read_bytes()
    rows = parse_semantic_payload(semantic_payload)
    if not rows:
        raise VerificationError("semantic inventory is empty")
    identities = [row[0] for row in rows]
    if len(set(identities)) != len(identities):
        raise VerificationError("semantic inventory duplicates item_key")
    # 先校验全部条目，防止非法代码进入编译器，或半途静默跳过未知形态。
    probes = [probe_source(row[3], row[5]) for row in rows]
    inputs = {str(path): digest(path) for path in (artifact, arguments.rustc)}
    inputs[str(semantic)] = hashlib.sha256(semantic_payload).hexdigest()
    version = subprocess.run([str(arguments.rustc), "--version"], capture_output=True, timeout=30)
    if version.returncode:
        raise VerificationError("rustc --version failed")
    output.mkdir(parents=True)
    results = []
    for index, (row, (source, scope)) in enumerate(zip(rows, probes)):
        item = {"item_key": row[0], "target": row[5], "rust_shape": row[3], "scope": scope}
        if not source:
            item.update(status="NOT_RUST_TARGET", exit_code=None)
            results.append(item)
            continue
        name = f"probe_{index:02d}"
        source_path = output / f"{name}.rs"
        source_path.write_text(source, encoding="utf-8", newline="\n")
        command = [str(arguments.rustc), "--edition=2021", "--crate-type=lib", "--emit=metadata",
                   "--crate-name", f"jdbc42_{name}", "-L", f"dependency={dependency}",
                   "--extern", f"druid_rdbc={artifact}", str(source_path),
                   "-o", str(output / f"{name}.rmeta")]
        try:
            completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
            raw, code = completed.stdout, completed.returncode
            status = "PASS" if code == 0 else "FAIL"
        except subprocess.TimeoutExpired as error:
            raw, code, status = error.stdout or b"", None, "ERROR_TIMEOUT"
        log = output / f"{name}.log"
        log.write_bytes(raw)
        item.update(status=status, exit_code=code, command=command, source=source_path.name,
                    source_sha256=digest(source_path), log=log.name, log_sha256=digest(log))
        results.append(item)
    changed = [path for path, sha in inputs.items() if not Path(path).is_file() or digest(Path(path)) != sha]
    summary = {
        "status": "PASS_PUBLIC_ENTRIES_ONLY" if not changed and all(
            item["status"] in {"PASS", "NOT_RUST_TARGET"} for item in results
        ) else "FAIL_PUBLIC_ENTRIES",
        "observed_utc": datetime.now(timezone.utc).isoformat(),
        "semantic_rows": len(rows), "passed": sum(x["status"] == "PASS" for x in results),
        "failed": sum(x["status"] == "FAIL" for x in results),
        "errors": sum(x["status"].startswith("ERROR") for x in results),
        "not_rust_target": sum(x["status"] == "NOT_RUST_TARGET" for x in results),
        "input_sha256": inputs, "changed_inputs": changed,
        "rustc_version": version.stdout.decode("utf-8", errors="replace").strip(),
        "artifact_source_provenance_certified": False,
        "full_shape_or_behavior_certified": False, "task8_complete": False,
        "results": results,
    }
    (output / "summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n",
    )
    return summary


def main() -> int:
    """运行公开入口诊断；语义、来源、行为与最终验收必须另行执行。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semantic", type=Path, default=Path(__file__).resolve().parents[1] / DEFAULT_SEMANTIC)
    parser.add_argument("--rustc", type=Path, required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--dependency-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        summary = verify(arguments)
    except (VerificationError, OSError, subprocess.SubprocessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PUBLIC_ENTRY_PASS={summary['passed']} FAIL={summary['failed']} ERROR={summary['errors']} EXCLUDED={summary['not_rust_target']}")
    return 0 if summary["status"] == "PASS_PUBLIC_ENTRIES_ONLY" else 1


if __name__ == "__main__":
    sys.exit(main())
