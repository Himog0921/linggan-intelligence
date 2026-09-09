#!/usr/bin/env python3
"""Isolated production-shape proof for the local Leiden compute route.

This creates synthetic, normalized vectors only.  It proves resource and
restart behaviour, never Chinese semantic correctness or Topic eligibility.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import signal
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

import numpy as np


REPO_ROOT = Path(__file__).resolve().parents[2]
APP_DIR = REPO_ROOT / "apps/comment-semantics"
PROGRAM = APP_DIR / "comment_semantics.py"
POLICY = APP_DIR / "cluster-policy.json"
PYTHON = Path(os.environ.get("COMMENT_SEMANTICS_PYTHON", APP_DIR / ".venv/bin/python"))
PROTOCOL = "ci-auto-semantics.v1"
REPORT_SCHEMA = "ci-auto-semantics-capacity-report.v1"
DEFAULT_REPORT = REPO_ROOT / "artifacts/private/comment-semantics/capacity-report.synthetic.json"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while block := handle.read(4 * 1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def controlled_env() -> dict[str, str]:
    result = dict(os.environ)
    for key in (
        "DATABASE_URL",
        "POSTGRES_URL",
        "POSTGRESQL_URL",
        "PGHOST",
        "PGPORT",
        "PGUSER",
        "PGPASSWORD",
        "PGDATABASE",
        "LINGGAN_DATABASE_URL",
    ):
        result.pop(key, None)
    return result


def write_vectors(path: Path, count: int, dimension: int, seed: int) -> None:
    """Generate and flush high-dimensional data in bounded chunks."""
    rng = np.random.default_rng(seed)
    centers = rng.normal(size=(20, dimension)).astype(np.float32)
    centers /= np.linalg.vector_norm(centers, axis=1, keepdims=True)
    vectors = np.memmap(path, dtype="<f4", mode="w+", shape=(count, dimension))
    for start in range(0, count, 256):
        end = min(count, start + 256)
        labels = np.arange(start, end) % len(centers)
        block = centers[labels] + rng.normal(0, 0.04, (end - start, dimension)).astype(np.float32)
        block /= np.linalg.vector_norm(block, axis=1, keepdims=True)
        vectors[start:end] = block
    vectors.flush()
    del vectors


def make_request(work_dir: Path, atom_ids: list[str], dimension: int, model_version: str, seed: int) -> dict:
    vector_path = work_dir / "vectors.f32"
    write_vectors(vector_path, len(atom_ids), dimension, seed)
    return {
        "protocolVersion": PROTOCOL,
        "workDir": str(work_dir),
        "snapshot": {
            "runRef": str(uuid.uuid4()),
            "kind": "problem",
            "space": {
                "model": "synthetic-capacity-only",
                "version": model_version,
                "dimension": dimension,
                "normalization": "l2",
            },
            "atomIds": atom_ids,
            "vectorPath": "vectors.f32",
            "snapshotHash": sha256_file(vector_path),
            "policy": {
                "version": "ci-auto-cluster.policy.v1",
                "sha256": sha256_file(POLICY),
                "mode": "leiden",
            },
        },
    }


def invoke(request: dict, timeout: int = 1805) -> tuple[dict, float]:
    started = time.monotonic()
    completed = subprocess.run(
        [str(PYTHON), str(PROGRAM)],
        input=json.dumps(request),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=controlled_env(),
        timeout=timeout,
        check=False,
    )
    elapsed = time.monotonic() - started
    if completed.returncode != 0:
        raise RuntimeError(f"compute failed rc={completed.returncode}: {completed.stderr}\n{completed.stdout}")
    return json.loads(completed.stdout), elapsed


def file_contains(path: Path, needle: bytes) -> bool:
    carry = b""
    with path.open("rb") as handle:
        while block := handle.read(1024 * 1024):
            if needle in carry + block:
                return True
            carry = block[-len(needle) :]
    return False


def validate_summary(work_dir: Path, summary: dict, expected_count: int, recovery: bool = False) -> None:
    if set(summary) != {"protocolVersion", "runRef", "snapshotHash", "resultPath", "resultHash", "bytes"}:
        raise RuntimeError(f"unexpected bounded stdout schema: {sorted(summary)}")
    result_path = work_dir / summary["resultPath"]
    if result_path.resolve().parent != work_dir.resolve():
        raise RuntimeError("result escaped controlled work directory")
    if sha256_file(result_path) != summary["resultHash"]:
        raise RuntimeError("result hash mismatch")
    if summary["bytes"] != result_path.stat().st_size:
        raise RuntimeError("result byte count mismatch")
    # Avoid loading a 100k-neighbor artifact into the verifier. These fixed
    # fields prove the intended high-dimensional, full-Leiden path was emitted.
    required = [
        b'"selectedAtomCount":' + str(expected_count).encode(),
        b'"dimension":1536',
        b'"mode":"leiden"',
        b'"leiden"',
        b'"resultHasTopicDefinition":false',
    ]
    if recovery:
        required.append(b'"checkpointRecovery":true')
    for token in required:
        if not file_contains(result_path, token):
            raise RuntimeError(f"result omitted required artifact token {token!r}")


def cleanup_stage(work_dir: Path) -> None:
    # Every path is created by this test below its private TemporaryDirectory.
    for child in work_dir.iterdir():
        child.unlink()
    work_dir.rmdir()


def run_stage(root: Path, name: str, atom_ids: list[str], version: str, seed: int, policy_hash: str) -> dict:
    work_dir = root / f"ci-auto-semantics-capacity-{name}"
    work_dir.mkdir()
    request = make_request(work_dir, atom_ids, 1536, version, seed)
    summary, elapsed = invoke(request)
    validate_summary(work_dir, summary, len(atom_ids))
    result = {
        "stage": name,
        "atoms": len(atom_ids),
        "dimension": 1536,
        "spaceVersion": version,
        "policySha256": policy_hash,
        "runRef": request["snapshot"]["runRef"],
        "vectorSnapshotHash": request["snapshot"]["snapshotHash"],
        "resultHash": summary["resultHash"],
        "resultBytes": summary["bytes"],
        "wallSeconds": round(elapsed, 3),
    }
    print(json.dumps(result, separators=(",", ":")))
    cleanup_stage(work_dir)
    return result


def run_interrupt_recovery(root: Path, atom_ids: list[str], policy_hash: str) -> dict:
    work_dir = root / "ci-auto-semantics-capacity-interrupt"
    work_dir.mkdir()
    request = make_request(work_dir, atom_ids, 1536, "v1", 24)
    process = subprocess.Popen(
        [str(PYTHON), str(PROGRAM)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=controlled_env(),
    )
    assert process.stdin is not None
    process.stdin.write(json.dumps(request))
    process.stdin.close()
    process.stdin = None
    checkpoint = work_dir / "checkpoint.json"
    deadline = time.monotonic() + 30
    while not checkpoint.exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    if not checkpoint.exists():
        process.kill()
        raise RuntimeError("process never persisted its running checkpoint")
    process.send_signal(signal.SIGTERM)
    stdout, stderr = process.communicate(timeout=60)
    if process.returncode != 130:
        raise RuntimeError(f"SIGTERM did not produce controlled interruption rc={process.returncode}: {stderr}\n{stdout}")
    interrupted = json.loads(stdout)
    if interrupted.get("status") != "interrupted":
        raise RuntimeError(f"unexpected interruption output: {interrupted}")
    checkpoint_data = json.loads(checkpoint.read_text(encoding="utf-8"))
    if checkpoint_data.get("status") != "interrupted":
        raise RuntimeError(f"checkpoint was not marked interrupted: {checkpoint_data}")
    summary, elapsed = invoke(request)
    validate_summary(work_dir, summary, len(atom_ids), recovery=True)
    result = {
        "stage": "interruption-recovery",
        "atoms": len(atom_ids),
        "dimension": 1536,
        "spaceVersion": "v1",
        "policySha256": policy_hash,
        "runRef": request["snapshot"]["runRef"],
        "vectorSnapshotHash": request["snapshot"]["snapshotHash"],
        "resultHash": summary["resultHash"],
        "resultBytes": summary["bytes"],
        "wallSeconds": round(elapsed, 3),
        "interrupted": True,
        "resumed": True,
    }
    print(json.dumps(result, separators=(",", ":")))
    cleanup_stage(work_dir)
    return result


def write_report(path: Path, report: dict) -> None:
    """Atomically retain synthetic capacity evidence outside version control."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--full", action="store_true", help="run all 100k/1536 production-shape stages")
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT,
        help="ignored local JSON evidence path written after a passing full proof",
    )
    arguments = parser.parse_args()
    if not PYTHON.exists():
        raise RuntimeError("run scripts/runtime/prepare-comment-semantics.sh --install first")
    if not arguments.full:
        print("capacity proof is intentionally opt-in; rerun with --full for 100k/1536 local synthetic stages")
        return 0
    policy_hash = sha256_file(POLICY)
    stages: list[dict] = []
    with tempfile.TemporaryDirectory(prefix="ci-auto-semantics-capacity-root-") as directory:
        root = Path(directory)
        ids = [str(uuid.uuid5(uuid.NAMESPACE_URL, f"ci-auto-semantics-capacity:{index}")) for index in range(101_000)]
        stages.append(run_stage(root, "baseline-100k", ids[:100_000], "v1", 17, policy_hash))
        stages.append(run_stage(root, "plus-1k", ids, "v1", 18, policy_hash))
        # Source withdrawal removes a fixed 1k slice while preserving every
        # surviving identifier's exact order in this frozen snapshot.
        withdrawn = ids[:50_000] + ids[51_000:]
        stages.append(run_stage(root, "withdraw-1k", withdrawn, "v1", 19, policy_hash))
        stages.append(run_interrupt_recovery(root, withdrawn, policy_hash))
        stages.append(run_stage(root, "new-space-v2", withdrawn, "v2", 20, policy_hash))
    if sha256_file(POLICY) != policy_hash:
        raise RuntimeError("policy changed while the capacity proof was running")
    if any(stage["policySha256"] != policy_hash for stage in stages):
        raise RuntimeError("capacity stage policy binding mismatch")
    write_report(
        arguments.report,
        {
            "schemaVersion": REPORT_SCHEMA,
            "status": "passed",
            "policy": {
                "version": "ci-auto-cluster.policy.v1",
                "sha256": policy_hash,
                "path": "apps/comment-semantics/cluster-policy.json",
            },
            "route": "leiden",
            "syntheticOnly": True,
            "limitations": [
                "Proves local resource and recovery behavior only.",
                "Does not prove semantic correctness, production data eligibility, or Topic definition.",
            ],
            "environment": {
                "python": platform.python_version(),
                "machine": platform.machine(),
                "system": platform.system(),
            },
            "stages": stages,
        },
    )
    print(json.dumps({"reportPath": str(arguments.report), "policySha256": policy_hash, "stages": len(stages)}, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
