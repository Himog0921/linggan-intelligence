#!/usr/bin/env python3
"""Local contract tests for the bounded CI-AUTO semantic compute process."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import unittest
import uuid
from pathlib import Path

import numpy as np


REPO_ROOT = Path(__file__).resolve().parents[2]
APP_DIR = REPO_ROOT / "apps/comment-semantics"
PROGRAM = APP_DIR / "comment_semantics.py"
POLICY_PATH = APP_DIR / "cluster-policy.json"
PYTHON = Path(os.environ.get("COMMENT_SEMANTICS_PYTHON", APP_DIR / ".venv/bin/python"))
PROTOCOL = "ci-auto-semantics.v1"


def controlled_env(**extra: str) -> dict[str, str]:
    environment = dict(os.environ)
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
        environment.pop(key, None)
    environment.update(extra)
    return environment


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_request(work_dir: Path, mode: str = "compare", count: int = 90, dimension: int = 12) -> dict:
    rng = np.random.default_rng(17)
    centers = np.zeros((3, dimension), dtype=np.float32)
    for index in range(3):
        centers[index, index] = 1.0
    vectors = np.vstack(
        [centers[index % 3] + rng.normal(0, 0.02, dimension) for index in range(count)]
    ).astype("<f4")
    vectors /= np.linalg.vector_norm(vectors, axis=1, keepdims=True)
    vector_path = work_dir / "vectors.f32"
    vectors.tofile(vector_path)
    return {
        "protocolVersion": PROTOCOL,
        "workDir": str(work_dir),
        "snapshot": {
            "runRef": str(uuid.uuid4()),
            "kind": "problem",
            "space": {"model": "test-embedding", "version": "v1", "dimension": dimension, "normalization": "l2"},
            "atomIds": [str(uuid.uuid4()) for _ in range(count)],
            "vectorPath": "vectors.f32",
            # The fixed v1 wire contract binds this field to the raw mmap bytes.
            "snapshotHash": sha256_file(vector_path),
            "policy": {
                "version": "ci-auto-cluster.policy.v1",
                "sha256": sha256_file(POLICY_PATH),
                "mode": mode,
            },
        },
    }


def run_request(request: dict, environment: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(PYTHON), str(PROGRAM)],
        input=json.dumps(request),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment or controlled_env(),
        check=False,
    )


class CommentSemanticsContractTest(unittest.TestCase):
    def require_runtime(self) -> None:
        self.assertTrue(PYTHON.exists(), f"controlled runtime missing: {PYTHON}; run prepare-comment-semantics.sh --install")

    def test_self_check_loads_all_real_algorithms(self) -> None:
        self.require_runtime()
        completed = subprocess.run(
            [str(PYTHON), str(PROGRAM), "--self-check"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            env=controlled_env(),
        )
        metadata = json.loads(completed.stdout)
        self.assertEqual(metadata["protocolVersion"], PROTOCOL)
        self.assertEqual(metadata["policySha256"], sha256_file(POLICY_PATH))
        self.assertEqual(metadata["threads"], 4)
        for dependency in ("faiss", "hdbscan", "igraph", "leidenalg"):
            self.assertNotEqual(metadata[dependency], "unknown")

    def test_compare_result_is_bounded_and_conservative(self) -> None:
        self.require_runtime()
        with tempfile.TemporaryDirectory(prefix="ci-auto-semantics-contract-") as directory:
            work_dir = Path(directory)
            request = write_request(work_dir, mode="compare")
            completed = run_request(request)
            self.assertEqual(completed.returncode, 0, completed.stderr + completed.stdout)
            # stdout is a bounded handoff only; 100k neighbors are not buffered there.
            summary = json.loads(completed.stdout)
            self.assertEqual(
                set(summary), {"protocolVersion", "runRef", "snapshotHash", "resultPath", "resultHash", "bytes"}
            )
            self.assertEqual(summary["resultPath"], "result.json")
            self.assertLess(len(completed.stdout), 512)
            result_path = work_dir / summary["resultPath"]
            self.assertEqual(sha256_file(result_path), summary["resultHash"])
            manifest = json.loads((work_dir / "manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(manifest["result"]["sha256"], summary["resultHash"])
            self.assertEqual(manifest["result"]["bytes"], summary["bytes"])
            self.assertEqual(manifest["input"]["sourceAtomCount"], 90)
            self.assertEqual(set(manifest["assignments"]), {"hdbscan", "leiden"})
            result = json.loads(result_path.read_text(encoding="utf-8"))
            self.assertFalse(result["artifact"]["resultHasTopicDefinition"])
            self.assertEqual(result["input"]["kind"], "problem")
            self.assertEqual(result["input"]["sourceAtomCount"], 90)
            self.assertEqual(result["quality"]["ann"]["hardGate"], "passed")
            self.assertEqual(set(result["assignments"]), {"hdbscan", "leiden"})
            expected_ids = request["snapshot"]["atomIds"]
            for route in result["assignments"].values():
                self.assertEqual([entry["atomId"] for entry in route], expected_ids)
                self.assertEqual(len({entry["atomId"] for entry in route}), len(expected_ids))
            self.assertEqual([entry["atomId"] for entry in result["neighbors"]], expected_ids)
            for entry in result["neighbors"]:
                self.assertEqual(len(entry["neighbors"]), 20)
                self.assertNotIn(entry["atomId"], [neighbor["atomId"] for neighbor in entry["neighbors"]])
            self.assertEqual(result["artifact"]["secondEmbeddingStability"]["status"], "not_configured")

    def test_rejects_hash_duplicate_id_and_path_escape(self) -> None:
        self.require_runtime()
        cases: list[tuple[str, callable]] = [
            ("hash", lambda request, work: request["snapshot"].__setitem__("snapshotHash", "0" * 64)),
            ("duplicate", lambda request, work: request["snapshot"].__setitem__("atomIds", [request["snapshot"]["atomIds"][0]] * len(request["snapshot"]["atomIds"]))),
            ("escape", lambda request, work: request["snapshot"].__setitem__("vectorPath", str(Path("/tmp") / "vectors.f32"))),
        ]
        for name, mutate in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory(prefix="ci-auto-semantics-contract-") as directory:
                work_dir = Path(directory)
                request = write_request(work_dir, mode="leiden", count=24)
                mutate(request, work_dir)
                completed = run_request(request)
                self.assertEqual(completed.returncode, 2, completed.stderr + completed.stdout)
                failure = json.loads(completed.stdout)
                self.assertEqual(failure["status"], "rejected")
                self.assertFalse((work_dir / "result.json").exists())

    def test_recovery_accepts_only_matching_interrupted_checkpoint(self) -> None:
        self.require_runtime()
        with tempfile.TemporaryDirectory(prefix="ci-auto-semantics-contract-") as directory:
            work_dir = Path(directory)
            request = write_request(work_dir, mode="leiden", count=45)
            checkpoint = {
                "protocolVersion": PROTOCOL,
                "runRef": request["snapshot"]["runRef"],
                "snapshotHash": request["snapshot"]["snapshotHash"],
                "policySha256": request["snapshot"]["policy"]["sha256"],
                "status": "interrupted",
                "stage": "building_hnsw",
            }
            (work_dir / "checkpoint.json").write_text(json.dumps(checkpoint), encoding="utf-8")
            completed = run_request(request)
            self.assertEqual(completed.returncode, 0, completed.stderr + completed.stdout)
            result = json.loads((work_dir / "result.json").read_text(encoding="utf-8"))
            self.assertTrue(result["artifact"]["checkpointRecovery"])
            self.assertFalse((work_dir / "checkpoint.json").exists())

    def test_database_credentials_are_rejected_before_compute(self) -> None:
        self.require_runtime()
        with tempfile.TemporaryDirectory(prefix="ci-auto-semantics-contract-") as directory:
            request = write_request(Path(directory), mode="leiden", count=24)
            completed = run_request(request, controlled_env(PGPASSWORD="not-accepted"))
            self.assertEqual(completed.returncode, 2)
            self.assertEqual(json.loads(completed.stdout)["status"], "rejected")


if __name__ == "__main__":
    unittest.main(verbosity=2)
