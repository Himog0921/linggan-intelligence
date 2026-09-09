#!/usr/bin/env python3
"""Bounded, local-only semantic clustering for CI-AUTO-004.

The process deliberately knows no comment text, database address, or Topic
schema.  Its input is a frozen vector snapshot and its only durable output is
one JSON artifact below the caller-created work directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import signal
import stat
import sys
import tempfile
import threading
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

# These must be set before NumPy, Faiss, SciPy, or OpenMP-backed extensions load.
THREADS = 4
for _thread_env in (
    "OMP_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "MKL_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
    "NUMEXPR_NUM_THREADS",
):
    os.environ[_thread_env] = str(THREADS)

import faiss  # noqa: E402
import hdbscan  # noqa: E402
import igraph as ig  # noqa: E402
import leidenalg  # noqa: E402
import numpy as np  # noqa: E402
import psutil  # noqa: E402
from sklearn.metrics import adjusted_rand_score, silhouette_score  # noqa: E402


PROTOCOL_VERSION = "ci-auto-semantics.v1"
POLICY_VERSION = "ci-auto-cluster.policy.v1"
APP_DIR = Path(__file__).resolve().parent
POLICY_PATH = APP_DIR / "cluster-policy.json"
CHECKPOINT_NAME = "checkpoint.json"
RESULT_NAME = "result.json"
MANIFEST_NAME = "manifest.json"
ALLOWED_KINDS = frozenset(
    {"problem", "need", "solution", "stance", "story", "quote", "emotion"}
)
ALLOWED_MODES = frozenset({"compare", "hdbscan", "leiden"})
MAX_DIMENSION = 8192
CHUNK_BYTES = 32 * 1024 * 1024


class ContractError(ValueError):
    """The caller gave data outside the fixed compute protocol."""


class ResourceExceeded(RuntimeError):
    """The resource guard found a hard budget violation."""


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode(
        "utf-8"
    )


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path, chunk_size: int = 4 * 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while block := handle.read(chunk_size):
            digest.update(block)
    return digest.hexdigest()


def atom_ids_hash(atom_ids: Sequence[str]) -> str:
    # Length prefixes prevent ambiguous concatenation without carrying text data.
    digest = hashlib.sha256()
    for atom_id in atom_ids:
        encoded = atom_id.encode("ascii")
        digest.update(len(encoded).to_bytes(2, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def json_file_atomic(path: Path, value: Any) -> None:
    temp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with temp.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temp, path)


def load_controlled_policy() -> tuple[dict[str, Any], str]:
    try:
        raw = POLICY_PATH.read_bytes()
        policy = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"controlled policy cannot be read: {error}") from error
    if not isinstance(policy, dict) or policy.get("policyVersion") != POLICY_VERSION:
        raise ContractError("controlled policy version is not ci-auto-cluster.policy.v1")
    return policy, sha256_bytes(raw)


def is_sha256(value: Any) -> bool:
    if not isinstance(value, str) or len(value) != 64:
        return False
    return all(character in "0123456789abcdef" for character in value)


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    missing = expected - actual
    extra = actual - expected
    if missing or extra:
        parts: list[str] = []
        if missing:
            parts.append(f"missing={sorted(missing)}")
        if extra:
            parts.append(f"unexpected={sorted(extra)}")
        raise ContractError(f"{label} keys are not fixed: {', '.join(parts)}")


def resolve_child(work_dir: Path, raw_path: Any, label: str) -> Path:
    if not isinstance(raw_path, str) or not raw_path:
        raise ContractError(f"{label} must be a non-empty path")
    candidate = Path(raw_path)
    if not candidate.is_absolute():
        candidate = work_dir / candidate
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise ContractError(f"{label} must name an existing path below workDir") from error
    try:
        resolved.relative_to(work_dir)
    except ValueError as error:
        raise ContractError(f"{label} must resolve below workDir") from error
    return resolved


@dataclass(frozen=True)
class Snapshot:
    run_ref: str
    kind: str
    model: str
    version: str
    dimension: int
    normalization: str
    atom_ids: tuple[str, ...]
    vector_path: Path
    snapshot_hash: str
    mode: str
    atom_ids_hash: str


def parse_input(document: Any, policy_hash: str) -> tuple[Path, Snapshot]:
    if not isinstance(document, dict):
        raise ContractError("stdin must contain one JSON object")
    require_exact_keys(document, {"protocolVersion", "workDir", "snapshot"}, "request")
    if document["protocolVersion"] != PROTOCOL_VERSION:
        raise ContractError("unsupported protocolVersion")
    raw_work_dir = document["workDir"]
    if not isinstance(raw_work_dir, str) or not raw_work_dir:
        raise ContractError("workDir must be a non-empty absolute path")
    try:
        work_dir = Path(raw_work_dir).resolve(strict=True)
    except OSError as error:
        raise ContractError("workDir must be an existing directory") from error
    if not work_dir.is_absolute() or not work_dir.is_dir():
        raise ContractError("workDir must be an existing directory")
    if not work_dir.name.startswith("ci-auto-semantics-"):
        raise ContractError("workDir must be a caller-created ci-auto-semantics-* directory")

    snapshot = document["snapshot"]
    if not isinstance(snapshot, dict):
        raise ContractError("snapshot must be an object")
    require_exact_keys(
        snapshot,
        {"runRef", "kind", "space", "atomIds", "vectorPath", "snapshotHash", "policy"},
        "snapshot",
    )
    if not isinstance(snapshot["runRef"], str):
        raise ContractError("snapshot.runRef must be a UUID")
    try:
        run_ref = str(uuid.UUID(snapshot["runRef"]))
    except (ValueError, AttributeError) as error:
        raise ContractError("snapshot.runRef must be a UUID") from error
    kind = snapshot["kind"]
    if kind not in ALLOWED_KINDS:
        raise ContractError("snapshot.kind is not an allowed independent atom kind")

    space = snapshot["space"]
    if not isinstance(space, dict):
        raise ContractError("snapshot.space must be an object")
    require_exact_keys(space, {"model", "version", "dimension", "normalization"}, "snapshot.space")
    if not isinstance(space["model"], str) or not space["model"].strip():
        raise ContractError("snapshot.space.model must be non-empty")
    if not isinstance(space["version"], str) or not space["version"].strip():
        raise ContractError("snapshot.space.version must be non-empty")
    dimension = space["dimension"]
    if not isinstance(dimension, int) or isinstance(dimension, bool) or not 1 <= dimension <= MAX_DIMENSION:
        raise ContractError(f"snapshot.space.dimension must be an integer from 1 to {MAX_DIMENSION}")
    if space["normalization"] != "l2":
        raise ContractError("snapshot.space.normalization must be l2")

    atom_ids_raw = snapshot["atomIds"]
    if not isinstance(atom_ids_raw, list) or not atom_ids_raw:
        raise ContractError("snapshot.atomIds must be a non-empty UUID array")
    atom_ids: list[str] = []
    seen: set[str] = set()
    for ordinal, atom_id in enumerate(atom_ids_raw):
        if not isinstance(atom_id, str):
            raise ContractError(f"snapshot.atomIds[{ordinal}] must be a UUID")
        try:
            canonical_id = str(uuid.UUID(atom_id))
        except ValueError as error:
            raise ContractError(f"snapshot.atomIds[{ordinal}] must be a UUID") from error
        if canonical_id != atom_id:
            raise ContractError(f"snapshot.atomIds[{ordinal}] must be canonical lower-case UUID")
        if canonical_id in seen:
            raise ContractError("snapshot.atomIds must be unique")
        seen.add(canonical_id)
        atom_ids.append(canonical_id)

    vector_path = resolve_child(work_dir, snapshot["vectorPath"], "snapshot.vectorPath")
    vector_stat = vector_path.stat()
    if not stat.S_ISREG(vector_stat.st_mode):
        raise ContractError("snapshot.vectorPath must name a regular float32 file")
    expected_bytes = len(atom_ids) * dimension * np.dtype("<f4").itemsize
    if vector_stat.st_size != expected_bytes:
        raise ContractError(
            f"vector file size {vector_stat.st_size} does not equal {len(atom_ids)}*{dimension}*4={expected_bytes}"
        )
    if not is_sha256(snapshot["snapshotHash"]):
        raise ContractError("snapshot.snapshotHash must be a lower-case SHA-256 hex string")
    # In this wire format snapshotHash is the canonical hash of the frozen raw
    # float32 snapshot. Rust records a separate ordered-ID hash from the result.
    actual_hash = sha256_file(vector_path)
    if actual_hash != snapshot["snapshotHash"]:
        raise ContractError("snapshot.snapshotHash does not match vector file bytes")

    supplied_policy = snapshot["policy"]
    if not isinstance(supplied_policy, dict):
        raise ContractError("snapshot.policy must be an object")
    require_exact_keys(supplied_policy, {"version", "sha256", "mode"}, "snapshot.policy")
    if supplied_policy["version"] != POLICY_VERSION or supplied_policy["sha256"] != policy_hash:
        raise ContractError("snapshot.policy must match the controlled policy bytes")
    mode = supplied_policy["mode"]
    if mode not in ALLOWED_MODES:
        raise ContractError("snapshot.policy.mode must be compare, hdbscan, or leiden")

    return work_dir, Snapshot(
        run_ref=run_ref,
        kind=kind,
        model=space["model"].strip(),
        version=space["version"].strip(),
        dimension=dimension,
        normalization="l2",
        atom_ids=tuple(atom_ids),
        vector_path=vector_path,
        snapshot_hash=actual_hash,
        mode=mode,
        atom_ids_hash=atom_ids_hash(atom_ids),
    )


def require_no_database_credentials() -> None:
    prohibited = (
        "DATABASE_URL",
        "POSTGRES_URL",
        "POSTGRESQL_URL",
        "PGHOST",
        "PGPORT",
        "PGUSER",
        "PGPASSWORD",
        "PGDATABASE",
        "LINGGAN_DATABASE_URL",
    )
    present = [key for key in prohibited if os.environ.get(key)]
    if present:
        raise ContractError("database credentials are forbidden in semantic compute environment")


class ResourceGuard:
    def __init__(self, work_dir: Path, snapshot: Snapshot, policy: dict[str, Any], policy_hash: str):
        resource = policy["resource"]
        self.work_dir = work_dir
        self.snapshot = snapshot
        self.policy_hash = policy_hash
        self.max_rss = int(resource["maxRssBytes"])
        self.timeout_seconds = int(resource["timeoutSeconds"])
        self.started_at = time.monotonic()
        self.stage = "initialized"
        self._process = psutil.Process()
        self._stopped = threading.Event()
        self._failure: ResourceExceeded | None = None
        self._lock = threading.Lock()
        self.peak_rss = 0
        self.recovered_from_checkpoint = False
        self._watchdog = threading.Thread(target=self._watch, name="resource-guard", daemon=True)

    @property
    def checkpoint_path(self) -> Path:
        return self.work_dir / CHECKPOINT_NAME

    def start(self) -> None:
        self._recover_if_matching()
        # A crash between parsing and the first expensive native call remains
        # recoverable. Normal completion atomically removes this marker.
        self._checkpoint("running")
        self._watchdog.start()

    def stop(self) -> None:
        self._stopped.set()
        if self._watchdog.is_alive():
            self._watchdog.join(timeout=1)

    def elapsed_seconds(self) -> float:
        return time.monotonic() - self.started_at

    def _observe(self) -> tuple[int, float]:
        rss = self._process.memory_info().rss
        elapsed = self.elapsed_seconds()
        with self._lock:
            self.peak_rss = max(self.peak_rss, rss)
        return rss, elapsed

    def _checkpoint(self, status: str, detail: str | None = None) -> None:
        rss, elapsed = self._observe()
        payload: dict[str, Any] = {
            "protocolVersion": PROTOCOL_VERSION,
            "runRef": self.snapshot.run_ref,
            "snapshotHash": self.snapshot.snapshot_hash,
            "policySha256": self.policy_hash,
            "status": status,
            "stage": self.stage,
            "elapsedSeconds": round(elapsed, 6),
            "rssBytes": rss,
            "peakRssBytes": self.peak_rss,
        }
        if detail is not None:
            payload["detail"] = detail
        json_file_atomic(self.checkpoint_path, payload)

    def _recover_if_matching(self) -> None:
        if not self.checkpoint_path.exists():
            return
        try:
            previous = json.loads(self.checkpoint_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ContractError(f"existing checkpoint is unreadable: {error}") from error
        if not isinstance(previous, dict):
            raise ContractError("existing checkpoint is not an object")
        if (
            previous.get("protocolVersion") != PROTOCOL_VERSION
            or previous.get("runRef") != self.snapshot.run_ref
            or previous.get("snapshotHash") != self.snapshot.snapshot_hash
            or previous.get("policySha256") != self.policy_hash
        ):
            raise ContractError("existing checkpoint belongs to another run, snapshot, or policy")
        self.recovered_from_checkpoint = previous.get("status") in {
            "running",
            "interrupted",
            "resource_exceeded",
        }

    def _record_failure(self, reason: str, force_exit: bool = False) -> None:
        should_checkpoint = False
        with self._lock:
            if self._failure is None:
                self._failure = ResourceExceeded(reason)
                should_checkpoint = True
        if should_checkpoint:
            # Do this outside _lock: _checkpoint observes RSS and takes the
            # same lock while maintaining the high-water mark.
            self._checkpoint("resource_exceeded", reason)
        if force_exit:
            # A native Faiss/HDBSCAN/Leiden section might not return to
            # check(). The watchdog has already persisted its restart marker,
            # so a hard exit is the only honest enforcement of the budget.
            os._exit(137)

    def _watch(self) -> None:
        while not self._stopped.wait(0.2):
            rss, elapsed = self._observe()
            if rss > self.max_rss:
                self._record_failure(f"rss {rss} exceeded {self.max_rss}", force_exit=True)
                return
            if elapsed > self.timeout_seconds:
                self._record_failure(
                    f"elapsed {elapsed:.3f}s exceeded {self.timeout_seconds}s", force_exit=True
                )
                return

    def check(self, stage: str) -> None:
        self.stage = stage
        rss, elapsed = self._observe()
        with self._lock:
            failure = self._failure
        if failure is not None:
            raise failure
        if rss > self.max_rss:
            self._record_failure(f"rss {rss} exceeded {self.max_rss}")
            raise ResourceExceeded(f"rss {rss} exceeded {self.max_rss}")
        if elapsed > self.timeout_seconds:
            self._record_failure(f"elapsed {elapsed:.3f}s exceeded {self.timeout_seconds}s")
            raise ResourceExceeded(f"elapsed {elapsed:.3f}s exceeded {self.timeout_seconds}s")

    def interrupted(self, signal_name: str) -> None:
        self._checkpoint("interrupted", signal_name)


def chunk_rows(dimension: int) -> int:
    return max(1, CHUNK_BYTES // (dimension * np.dtype("<f4").itemsize))


def validate_vectors(snapshot: Snapshot, guard: ResourceGuard) -> np.memmap:
    vectors = np.memmap(
        snapshot.vector_path,
        dtype="<f4",
        mode="r",
        shape=(len(snapshot.atom_ids), snapshot.dimension),
        order="C",
    )
    tolerance = 1e-4
    for start in range(0, len(snapshot.atom_ids), chunk_rows(snapshot.dimension)):
        guard.check("validating_vectors")
        end = min(len(snapshot.atom_ids), start + chunk_rows(snapshot.dimension))
        block = np.asarray(vectors[start:end], dtype=np.float32)
        if not np.isfinite(block).all():
            raise ContractError(f"vector values are non-finite near ordinal {start}")
        norms = np.linalg.vector_norm(block, axis=1)
        invalid = np.flatnonzero(np.abs(norms - 1.0) > tolerance)
        if invalid.size:
            ordinal = start + int(invalid[0])
            raise ContractError(f"vector at ordinal {ordinal} is not L2 normalized")
    return vectors


def stratified_sample_indices(atom_count: int, max_atoms: int, seed: int) -> np.ndarray:
    """No text strata exist in this process; use stable ordinal buckets as strata.

    Rust has already chosen one kind, source and vector space.  The deterministic
    ordinal buckets retain coverage across its frozen order without recovering
    external facts or a source corpus here.
    """
    if atom_count <= max_atoms:
        return np.arange(atom_count, dtype=np.int64)
    bucket_count = min(max_atoms, 100)
    rng = np.random.default_rng(seed)
    result: list[np.ndarray] = []
    target_per_bucket = max_atoms // bucket_count
    remainder = max_atoms % bucket_count
    for bucket in range(bucket_count):
        start = (atom_count * bucket) // bucket_count
        end = (atom_count * (bucket + 1)) // bucket_count
        target = target_per_bucket + (1 if bucket < remainder else 0)
        chosen = rng.choice(np.arange(start, end, dtype=np.int64), size=target, replace=False)
        result.append(chosen)
    return np.sort(np.concatenate(result))


def build_hnsw_knn(vectors: np.memmap, policy: dict[str, Any], guard: ResourceGuard) -> tuple[np.ndarray, np.ndarray]:
    config = policy["knn"]
    count, dimension = vectors.shape
    k = min(int(config["k"]), max(0, count - 1))
    if k == 0:
        return np.empty((count, 0), dtype=np.int64), np.empty((count, 0), dtype=np.float32)
    faiss.omp_set_num_threads(int(policy["resource"]["threads"]))
    index = faiss.IndexHNSWFlat(dimension, int(config["hnswM"]), faiss.METRIC_INNER_PRODUCT)
    index.hnsw.efConstruction = int(config["efConstruction"])
    index.hnsw.efSearch = int(config["efSearch"])
    rows = chunk_rows(dimension)
    for start in range(0, count, rows):
        guard.check("building_hnsw")
        end = min(count, start + rows)
        index.add(np.ascontiguousarray(vectors[start:end], dtype=np.float32))

    neighbor_ids = np.empty((count, k), dtype=np.int64)
    neighbor_scores = np.empty((count, k), dtype=np.float32)
    search_rows = min(rows, 4096)
    for start in range(0, count, search_rows):
        guard.check("searching_hnsw")
        end = min(count, start + search_rows)
        scores, candidates = index.search(
            np.ascontiguousarray(vectors[start:end], dtype=np.float32), k + 1
        )
        for offset, query_id in enumerate(range(start, end)):
            keep = candidates[offset] != query_id
            ids = candidates[offset][keep][:k]
            values = scores[offset][keep][:k]
            if len(ids) != k or np.any(ids < 0):
                raise ResourceExceeded("HNSW did not return a complete non-self neighbor list")
            neighbor_ids[query_id] = ids
            neighbor_scores[query_id] = values
    return neighbor_ids, neighbor_scores


def exact_neighbors_for_queries(
    vectors: np.memmap, query_indices: np.ndarray, k: int, guard: ResourceGuard
) -> np.ndarray:
    """Streaming exact cosine top-k: q×block only, never an n×n matrix."""
    if k == 0:
        return np.empty((len(query_indices), 0), dtype=np.int64)
    query_vectors = np.ascontiguousarray(vectors[query_indices], dtype=np.float32)
    best_scores = np.full((len(query_indices), k), -np.inf, dtype=np.float32)
    best_ids = np.full((len(query_indices), k), -1, dtype=np.int64)
    block_rows = max(1, min(chunk_rows(vectors.shape[1]), 8192))
    for start in range(0, vectors.shape[0], block_rows):
        guard.check("exact_recall_reference")
        end = min(vectors.shape[0], start + block_rows)
        scores = query_vectors @ np.ascontiguousarray(vectors[start:end], dtype=np.float32).T
        block_ids = np.arange(start, end, dtype=np.int64)
        # Exclude self only when this block contains the query's ordinal.
        query_mask = (query_indices >= start) & (query_indices < end)
        if np.any(query_mask):
            rows = np.flatnonzero(query_mask)
            scores[rows, query_indices[rows] - start] = -np.inf
        candidate_scores = np.concatenate((best_scores, scores), axis=1)
        candidate_ids = np.concatenate(
            (best_ids, np.broadcast_to(block_ids, (len(query_indices), len(block_ids)))), axis=1
        )
        picks = np.argpartition(candidate_scores, -k, axis=1)[:, -k:]
        best_scores = np.take_along_axis(candidate_scores, picks, axis=1)
        best_ids = np.take_along_axis(candidate_ids, picks, axis=1)
    ordering = np.argsort(-best_scores, axis=1, kind="stable")
    return np.take_along_axis(best_ids, ordering, axis=1)


def ann_quality(
    vectors: np.memmap, neighbor_ids: np.ndarray, policy: dict[str, Any], guard: ResourceGuard
) -> dict[str, Any]:
    count = vectors.shape[0]
    k = neighbor_ids.shape[1]
    if k == 0:
        return {
            "status": "insufficient",
            "reason": "fewer than two atoms",
            "queries": 0,
            "k": 0,
            "recallAt20": None,
            "minimum": float(policy["quality"]["minRecallAt20"]),
            "hardGate": "insufficient",
        }
    query_count = min(count, int(policy["quality"]["recallQueries"]))
    rng = np.random.default_rng(int(policy["quality"]["seeds"][0]))
    query_indices = np.sort(rng.choice(count, size=query_count, replace=False)).astype(np.int64)
    exact = exact_neighbors_for_queries(vectors, query_indices, k, guard)
    recalls = [
        len(set(neighbor_ids[index].tolist()).intersection(exact[row].tolist())) / k
        for row, index in enumerate(query_indices)
    ]
    recall = float(np.mean(recalls))
    minimum = float(policy["quality"]["minRecallAt20"])
    return {
        "status": "measured",
        "queries": int(query_count),
        "k": int(k),
        "recallAt20": recall,
        "minimum": minimum,
        "hardGate": "passed" if recall >= minimum else "failed",
    }


def run_hdbscan(vectors: np.ndarray, seed: int, policy: dict[str, Any], guard: ResourceGuard) -> np.ndarray:
    guard.check(f"hdbscan_seed_{seed}")
    config = policy["hdbscan"]
    # hdbscan itself has no seed argument. Setting NumPy's process seed records
    # the fixed execution condition; repeat checks expose implementation drift.
    np.random.seed(seed)
    clusterer = hdbscan.HDBSCAN(
        min_cluster_size=int(config["minClusterSize"]),
        min_samples=int(config["minSamples"]),
        metric=config["metric"],
        # Pin a tree-backed implementation: the generic branch materializes a
        # pairwise distance matrix and is prohibited even for a comparison sample.
        algorithm="boruvka_kdtree",
        core_dist_n_jobs=int(policy["resource"]["threads"]),
        approx_min_span_tree=True,
        prediction_data=False,
    )
    return np.asarray(clusterer.fit_predict(vectors), dtype=np.int64)


def graph_from_knn(neighbor_ids: np.ndarray, neighbor_scores: np.ndarray, guard: ResourceGuard) -> ig.Graph:
    graph = ig.Graph(n=int(neighbor_ids.shape[0]), directed=False)
    for start in range(0, neighbor_ids.shape[0], 1024):
        guard.check("building_leiden_graph")
        end = min(neighbor_ids.shape[0], start + 1024)
        sources = np.repeat(np.arange(start, end, dtype=np.int64), neighbor_ids.shape[1])
        targets = neighbor_ids[start:end].reshape(-1)
        graph.add_edges(zip(sources.tolist(), targets.tolist()))
    # Cosine on L2-normalized vectors can be negative. Leiden requires
    # non-negative edge weights, so the fixed graph policy retains affinity only.
    weights = np.maximum(neighbor_scores.reshape(-1), 0.0).astype(np.float64, copy=False)
    graph.es["weight"] = weights.tolist()
    return graph


def run_leiden(graph: ig.Graph, seed: int, policy: dict[str, Any], guard: ResourceGuard) -> np.ndarray:
    guard.check(f"leiden_seed_{seed}")
    config = policy["leiden"]
    partition = leidenalg.find_partition(
        graph,
        leidenalg.RBConfigurationVertexPartition,
        weights="weight",
        resolution_parameter=float(config["resolution"]),
        n_iterations=int(config["nIterations"]),
        seed=seed,
    )
    return np.asarray(partition.membership, dtype=np.int64)


def label_permutation_signature(labels: np.ndarray) -> tuple[int, ...]:
    mapping: dict[int, int] = {}
    next_label = 0
    signature: list[int] = []
    for raw_label in labels.tolist():
        label = int(raw_label)
        if label == -1:
            signature.append(-1)
            continue
        if label not in mapping:
            mapping[label] = next_label
            next_label += 1
        signature.append(mapping[label])
    return tuple(signature)


def cluster_sizes(labels: np.ndarray) -> list[int]:
    members = labels[labels != -1]
    if not members.size:
        return []
    return sorted((int(value) for value in np.unique(members, return_counts=True)[1]), reverse=True)


def seed_quality(
    label_sets: dict[int, np.ndarray], policy: dict[str, Any], same_seed_repeat: np.ndarray
) -> dict[str, Any]:
    seeds = [int(seed) for seed in policy["quality"]["seeds"]]
    baseline = label_sets[seeds[0]]
    repeat_equivalent = label_permutation_signature(baseline) == label_permutation_signature(
        same_seed_repeat
    )
    noise_equivalent = bool(np.array_equal(baseline == -1, same_seed_repeat == -1))
    pairs: list[dict[str, Any]] = []
    aris: list[float] = []
    common_counts: list[int] = []
    for offset, left_seed in enumerate(seeds):
        for right_seed in seeds[offset + 1 :]:
            left = label_sets[left_seed]
            right = label_sets[right_seed]
            common = (left != -1) & (right != -1)
            common_count = int(common.sum())
            common_counts.append(common_count)
            ari: float | None = None
            if common_count >= 2:
                ari = float(adjusted_rand_score(left[common], right[common]))
                aris.append(ari)
            pairs.append(
                {
                    "leftSeed": left_seed,
                    "rightSeed": right_seed,
                    "commonNonNoise": common_count,
                    "ari": ari,
                }
            )
    min_common = int(policy["quality"]["minCommonCore"])
    common_core = min(common_counts) if common_counts else 0
    mean_ari = float(np.mean(aris)) if aris else None
    if common_core < min_common:
        perturbation = "insufficient"
    elif mean_ari is not None and mean_ari >= float(policy["quality"]["minStabilityAri"]):
        perturbation = "passed"
    else:
        perturbation = "failed"
    return {
        "sameSeed": {
            "seed": seeds[0],
            "repeats": int(policy["quality"]["sameSeedRepeats"]),
            "labelPermutationEquivalent": repeat_equivalent,
            "noiseIdentityEquivalent": noise_equivalent,
            "hardGate": "passed" if repeat_equivalent and noise_equivalent else "failed",
        },
        "perturbation": {
            "seeds": seeds,
            "pairs": pairs,
            "meanAri": mean_ari,
            "minimumAri": float(policy["quality"]["minStabilityAri"]),
            "commonCore": common_core,
            "minimumCommonCore": min_common,
            "gate": perturbation,
        },
    }


def cluster_geometry(vectors: np.ndarray, labels: np.ndarray, policy: dict[str, Any], guard: ResourceGuard) -> dict[str, Any]:
    non_noise = labels != -1
    unique = np.unique(labels[non_noise])
    if unique.size == 0:
        return {
            "clusters": 0,
            "noiseRate": 1.0,
            "clusterSizes": [],
            "centerCosine": {"clusterWeighted": None, "memberWeighted": None},
            "separation": {"silhouette": None, "nearestCenterGap": None, "status": "not_computable"},
        }
    # This holds one float32 center accumulator per actual cluster, never a
    # sample-by-sample distance matrix.
    label_positions = np.searchsorted(unique, labels[non_noise])
    selected = np.asarray(vectors[non_noise], dtype=np.float32)
    sums = np.zeros((unique.size, selected.shape[1]), dtype=np.float32)
    np.add.at(sums, label_positions, selected)
    counts = np.bincount(label_positions, minlength=unique.size).astype(np.int64)
    center_norms = np.linalg.vector_norm(sums, axis=1)
    centers = sums / np.maximum(center_norms[:, None], np.finfo(np.float32).eps)
    member_cosines = np.einsum("ij,ij->i", selected, centers[label_positions])
    per_cluster_means = np.bincount(
        label_positions, weights=member_cosines, minlength=unique.size
    ) / counts
    center_result = {
        "clusterWeighted": float(np.mean(per_cluster_means)),
        "memberWeighted": float(np.mean(member_cosines)),
    }
    if unique.size < 2 or selected.shape[0] < 3:
        separation = {"silhouette": None, "nearestCenterGap": None, "status": "not_computable"}
    else:
        guard.check("cluster_geometry")
        sample_limit = int(policy["quality"]["silhouetteSampleMaxAtoms"])
        rng = np.random.default_rng(int(policy["quality"]["seeds"][0]))
        sample_size = min(selected.shape[0], sample_limit)
        sample_indices = np.sort(rng.choice(selected.shape[0], sample_size, replace=False))
        sample_labels = label_positions[sample_indices]
        if np.unique(sample_labels).size < 2:
            silhouette: float | None = None
            silhouette_status = "not_computable"
        else:
            # Bounded at 1000 members by policy. It is an explicit diagnostic
            # sample; no full n² distance matrix is formed.
            silhouette = float(silhouette_score(selected[sample_indices], sample_labels, metric="euclidean"))
            silhouette_status = "estimated"
        center_index = faiss.IndexFlatIP(centers.shape[1])
        center_index.add(np.ascontiguousarray(centers, dtype=np.float32))
        similarity, _ = center_index.search(np.ascontiguousarray(centers, dtype=np.float32), 2)
        nearest_gap = float(np.mean(1.0 - similarity[:, 1]))
        separation = {
            "silhouette": silhouette,
            "silhouetteSample": int(sample_size),
            "nearestCenterGap": nearest_gap,
            "status": silhouette_status,
        }
    return {
        "clusters": int(unique.size),
        "noiseRate": float(1.0 - non_noise.mean()),
        "clusterSizes": cluster_sizes(labels),
        "centerCosine": center_result,
        "separation": separation,
    }


def route_report(
    route: str,
    labels_by_seed: dict[int, np.ndarray],
    vectors: np.ndarray,
    policy: dict[str, Any],
    guard: ResourceGuard,
    same_seed_repeat: np.ndarray,
) -> tuple[np.ndarray, dict[str, Any]]:
    canonical_seed = int(policy["quality"]["sameSeed"])
    labels = labels_by_seed[canonical_seed]
    seed_metrics = seed_quality(labels_by_seed, policy, same_seed_repeat)
    return labels, {
        "route": route,
        "seedMetrics": seed_metrics,
        "geometry": cluster_geometry(vectors, labels, policy, guard),
    }


def algorithm_agreement(left: np.ndarray, right: np.ndarray) -> dict[str, Any]:
    common = (left != -1) & (right != -1)
    count = int(common.sum())
    return {
        "commonNonNoise": count,
        "commonCoverage": float(count / len(left)) if len(left) else 0.0,
        "ari": float(adjusted_rand_score(left[common], right[common])) if count >= 2 else None,
        "noiseDisagreement": int(np.count_nonzero((left == -1) != (right == -1))),
        "status": "measured" if count >= 2 else "insufficient",
    }


def hard_gates(ann: dict[str, Any], routes: dict[str, dict[str, Any]]) -> dict[str, str]:
    input_gate = "passed"
    ann_gate = ann["hardGate"]
    same_seed = "passed"
    perturbation = "passed"
    for report in routes.values():
        same = report["seedMetrics"]["sameSeed"]["hardGate"]
        stability = report["seedMetrics"]["perturbation"]["gate"]
        if same == "failed":
            same_seed = "failed"
        if stability == "failed":
            perturbation = "failed"
        elif stability == "insufficient" and perturbation != "failed":
            perturbation = "insufficient"
    return {
        "inputConservation": input_gate,
        "annRecall": ann_gate,
        "sameSeed": same_seed,
        "perturbation": perturbation,
    }


def stable_selected_indices(snapshot: Snapshot, policy: dict[str, Any]) -> np.ndarray:
    if snapshot.mode != "compare":
        return np.arange(len(snapshot.atom_ids), dtype=np.int64)
    return stratified_sample_indices(
        len(snapshot.atom_ids), int(policy["hdbscan"]["compareSampleMaxAtoms"]), int(policy["quality"]["seeds"][0])
    )


def result_stream(
    path: Path,
    snapshot: Snapshot,
    policy_hash: str,
    policy: dict[str, Any],
    selected_indices: np.ndarray,
    labels: dict[str, np.ndarray],
    neighbor_ids: np.ndarray,
    neighbor_scores: np.ndarray,
    quality: dict[str, Any],
    guard: ResourceGuard,
    elapsed_seconds: float,
) -> tuple[str, int]:
    temp_path = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    selected_atom_ids = [snapshot.atom_ids[int(index)] for index in selected_indices]
    header = {
        "protocolVersion": PROTOCOL_VERSION,
        "runRef": snapshot.run_ref,
        "snapshotHash": snapshot.snapshot_hash,
        "policy": {"version": POLICY_VERSION, "sha256": policy_hash},
        "input": {
            "kind": snapshot.kind,
            "space": {
                "model": snapshot.model,
                "version": snapshot.version,
                "dimension": snapshot.dimension,
                "normalization": snapshot.normalization,
            },
            "sourceAtomCount": len(snapshot.atom_ids),
            "sourceAtomIdsHash": snapshot.atom_ids_hash,
            "selectedAtomCount": len(selected_indices),
            "selectedOrdinalHash": sha256_bytes(selected_indices.astype("<i8", copy=False).tobytes()),
            "atomIdsHash": atom_ids_hash(selected_atom_ids),
            "vectorSha256": snapshot.snapshot_hash,
            "mode": snapshot.mode,
        },
    }
    artifact = {
        "policyVersion": POLICY_VERSION,
        "resource": {
            "threads": int(policy["resource"]["threads"]),
            "maxRssBytes": int(policy["resource"]["maxRssBytes"]),
            "timeoutSeconds": int(policy["resource"]["timeoutSeconds"]),
            "peakRssBytes": int(guard.peak_rss),
            "elapsedSeconds": round(elapsed_seconds, 6),
        },
        "implementation": {
            "python": sys.version.split()[0],
            "faiss": getattr(faiss, "__version__", "unknown"),
            "hdbscan": getattr(hdbscan, "__version__", "0.8.44"),
            "igraph": getattr(ig, "__version__", "unknown"),
            "leidenalg": getattr(leidenalg, "__version__", "unknown"),
        },
        "checkpointRecovery": guard.recovered_from_checkpoint,
        "resultHasTopicDefinition": False,
        "secondEmbeddingStability": {"status": "not_configured"},
    }
    with temp_path.open("w", encoding="utf-8") as handle:
        handle.write("{")
        for key, value in header.items():
            handle.write(json.dumps(key, ensure_ascii=False))
            handle.write(":")
            handle.write(json.dumps(value, ensure_ascii=False, separators=(",", ":")))
            handle.write(",")
        handle.write('"assignments":{')
        for route_offset, (route, route_labels) in enumerate(labels.items()):
            if route_offset:
                handle.write(",")
            handle.write(json.dumps(route))
            handle.write(":[")
            for ordinal, label in enumerate(route_labels.tolist()):
                if ordinal:
                    handle.write(",")
                cluster = int(label)
                assignment = {
                    "atomId": selected_atom_ids[ordinal],
                    "cluster": None if cluster == -1 else cluster,
                    "noise": cluster == -1,
                }
                handle.write(json.dumps(assignment, ensure_ascii=False, separators=(",", ":")))
            handle.write("]")
        handle.write("},\"neighbors\":[")
        for ordinal, atom_id in enumerate(selected_atom_ids):
            if ordinal:
                handle.write(",")
            neighbor = {
                "atomId": atom_id,
                "neighbors": [
                    {
                        "atomId": selected_atom_ids[int(neighbor_id)],
                        "cosine": float(score),
                    }
                    for neighbor_id, score in zip(neighbor_ids[ordinal], neighbor_scores[ordinal])
                ],
            }
            handle.write(json.dumps(neighbor, ensure_ascii=False, separators=(",", ":")))
        handle.write("]")
        handle.write(',"quality":')
        handle.write(json.dumps(quality, ensure_ascii=False, separators=(",", ":")))
        handle.write(',"artifact":')
        handle.write(json.dumps(artifact, ensure_ascii=False, separators=(",", ":")))
        handle.write("}\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temp_path, path)
    return sha256_file(path), path.stat().st_size


def write_manifest(
    work_dir: Path,
    snapshot: Snapshot,
    policy_hash: str,
    selected_indices: np.ndarray,
    labels: dict[str, np.ndarray],
    neighbor_ids: np.ndarray,
    result_hash: str,
    byte_count: int,
) -> None:
    """Bounded companion for a Rust caller; it never replaces result.json."""
    selected_atom_ids = [snapshot.atom_ids[int(index)] for index in selected_indices]
    selected_hash = atom_ids_hash(selected_atom_ids)
    manifest = {
        "protocolVersion": PROTOCOL_VERSION,
        "runRef": snapshot.run_ref,
        "snapshotHash": snapshot.snapshot_hash,
        "policy": {"version": POLICY_VERSION, "sha256": policy_hash},
        "input": {
            "kind": snapshot.kind,
            "space": {
                "model": snapshot.model,
                "version": snapshot.version,
                "dimension": snapshot.dimension,
                "normalization": snapshot.normalization,
            },
            "sourceAtomCount": len(snapshot.atom_ids),
            "sourceAtomIdsHash": snapshot.atom_ids_hash,
            "selectedAtomCount": len(selected_indices),
            "selectedAtomIdsHash": selected_hash,
            "selectedOrdinalHash": sha256_bytes(selected_indices.astype("<i8", copy=False).tobytes()),
            "mode": snapshot.mode,
        },
        "assignments": {
            route: {"count": len(route_labels), "atomIdsHash": selected_hash}
            for route, route_labels in labels.items()
        },
        "neighbors": {
            "count": len(selected_indices),
            "atomIdsHash": selected_hash,
            "neighborsPerAtom": int(neighbor_ids.shape[1]),
            "selfEdges": 0,
        },
        "result": {"path": RESULT_NAME, "sha256": result_hash, "bytes": byte_count},
    }
    json_file_atomic(work_dir / MANIFEST_NAME, manifest)


def execute(document: Any) -> dict[str, Any]:
    require_no_database_credentials()
    policy, policy_hash = load_controlled_policy()
    work_dir, snapshot = parse_input(document, policy_hash)
    guard = ResourceGuard(work_dir, snapshot, policy, policy_hash)
    previous_handlers: dict[int, Any] = {}

    def on_signal(signum: int, _frame: Any) -> None:
        guard.interrupted(signal.Signals(signum).name)
        raise KeyboardInterrupt(signal.Signals(signum).name)

    for signum in (signal.SIGTERM, signal.SIGINT):
        previous_handlers[signum] = signal.getsignal(signum)
        signal.signal(signum, on_signal)
    guard.start()
    try:
        guard.check("validating_input")
        vectors = validate_vectors(snapshot, guard)
        selected_indices = stable_selected_indices(snapshot, policy)
        if len(selected_indices) == len(snapshot.atom_ids) and np.array_equal(
            selected_indices, np.arange(len(snapshot.atom_ids), dtype=np.int64)
        ):
            # Preserve the source float32 mmap for the production route. The
            # comparison route is bounded to 10,000 and may materialize it.
            selected_vectors = vectors
        else:
            selected_vectors = np.asarray(vectors[selected_indices], dtype=np.float32)
        selected_atom_count = len(selected_indices)
        if selected_atom_count != len(np.unique(selected_indices)):
            raise ContractError("selected snapshot indices are not unique")
        neighbor_ids, neighbor_scores = build_hnsw_knn(selected_vectors, policy, guard)
        ann = ann_quality(selected_vectors, neighbor_ids, policy, guard)

        seeds = [int(seed) for seed in policy["quality"]["seeds"]]
        routes: dict[str, dict[str, Any]] = {}
        final_labels: dict[str, np.ndarray] = {}
        if snapshot.mode in {"compare", "hdbscan"}:
            hdbscan_labels: dict[int, np.ndarray] = {}
            for seed in seeds:
                hdbscan_labels[seed] = run_hdbscan(selected_vectors, seed, policy, guard)
            # Explicit fixed same-seed repeat, independent of the three seed sample.
            repeat = run_hdbscan(selected_vectors, int(policy["quality"]["sameSeed"]), policy, guard)
            labels, report = route_report(
                "hdbscan", hdbscan_labels, selected_vectors, policy, guard, repeat
            )
            routes["hdbscan"] = report
            final_labels["hdbscan"] = labels
        if snapshot.mode in {"compare", "leiden"}:
            graph = graph_from_knn(neighbor_ids, neighbor_scores, guard)
            leiden_labels: dict[int, np.ndarray] = {}
            for seed in seeds:
                leiden_labels[seed] = run_leiden(graph, seed, policy, guard)
            repeat = run_leiden(graph, int(policy["quality"]["sameSeed"]), policy, guard)
            labels, report = route_report(
                "leiden", leiden_labels, selected_vectors, policy, guard, repeat
            )
            routes["leiden"] = report
            final_labels["leiden"] = labels

        quality: dict[str, Any] = {
            "ann": ann,
            "routes": routes,
            "gates": hard_gates(ann, routes),
            "secondEmbeddingStability": {"status": "not_configured"},
        }
        if "hdbscan" in final_labels and "leiden" in final_labels:
            quality["algorithmAgreement"] = algorithm_agreement(
                final_labels["hdbscan"], final_labels["leiden"]
            )
        else:
            quality["algorithmAgreement"] = {"status": "not_run"}

        guard.check("writing_result")
        result_path = work_dir / RESULT_NAME
        result_hash, byte_count = result_stream(
            result_path,
            snapshot,
            policy_hash,
            policy,
            selected_indices,
            final_labels,
            neighbor_ids,
            neighbor_scores,
            quality,
            guard,
            guard.elapsed_seconds(),
        )
        write_manifest(
            work_dir,
            snapshot,
            policy_hash,
            selected_indices,
            final_labels,
            neighbor_ids,
            result_hash,
            byte_count,
        )
        checkpoint = guard.checkpoint_path
        if checkpoint.exists():
            checkpoint.unlink()
        return {
            "protocolVersion": PROTOCOL_VERSION,
            "runRef": snapshot.run_ref,
            "snapshotHash": snapshot.snapshot_hash,
            "resultPath": RESULT_NAME,
            "resultHash": result_hash,
            "bytes": byte_count,
        }
    finally:
        guard.stop()
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)


def self_check() -> dict[str, Any]:
    policy, policy_hash = load_controlled_policy()
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "policyVersion": policy["policyVersion"],
        "policySha256": policy_hash,
        "threads": policy["resource"]["threads"],
        "faiss": getattr(faiss, "__version__", "unknown"),
        "hdbscan": getattr(hdbscan, "__version__", "0.8.44"),
        "igraph": getattr(ig, "__version__", "unknown"),
        "leidenalg": getattr(leidenalg, "__version__", "unknown"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--self-check", action="store_true")
    arguments = parser.parse_args()
    try:
        if arguments.self_check:
            print(json.dumps(self_check(), ensure_ascii=False, separators=(",", ":")))
            return 0
        if sys.stdin.isatty():
            raise ContractError("stdin must be the fixed semantic snapshot JSON")
        document = json.load(sys.stdin)
        print(json.dumps(execute(document), ensure_ascii=False, separators=(",", ":")))
        return 0
    except KeyboardInterrupt as error:
        print(json.dumps({"protocolVersion": PROTOCOL_VERSION, "status": "interrupted", "detail": str(error)}))
        return 130
    except (ContractError, ResourceExceeded, json.JSONDecodeError) as error:
        print(json.dumps({"protocolVersion": PROTOCOL_VERSION, "status": "rejected", "detail": str(error)}))
        return 2
    except Exception as error:  # Keep stdout protocol-only even if an extension rejects data.
        print(json.dumps({"protocolVersion": PROTOCOL_VERSION, "status": "failed", "detail": str(error)}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
