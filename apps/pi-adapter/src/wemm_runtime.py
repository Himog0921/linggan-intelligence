#!/usr/bin/env python3
"""One local, line-delimited executor for LOCAL-EMBEDDING-001.

It deliberately exposes no HTTP port and knows no research state.  The Rust research worker
owns lifecycle, receipt and retry semantics; this process only keeps the fixed WeMM model resident
and accepts bounded document-encoding requests on stdin.
"""

from __future__ import annotations

import argparse
import json
import os
import resource
import sys
import time
from typing import Any

os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
os.environ.setdefault("TRANSFORMERS_VERBOSITY", "error")

PROTOCOL = "linggan.wemm.v1"
MODEL_ID = "Tencent/WeMM-Embedding-2B"
DIMENSION = 512
MAX_TEXTS_PER_REQUEST = 32
MAX_TEXT_CHARS = 16_000


def emit(value: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def resources(torch_module: Any, backend: str) -> dict[str, Any]:
    """What this process is actually costing, for the qualification record.

    Peak RSS is the resident set of *this* process and says nothing about GPU-side memory, so the
    MPS figures are reported separately and only when the backend can produce them. System memory
    pressure and swap activity are deliberately absent rather than guessed: reading them needs a
    dependency this verified venv does not carry.
    """
    usage: dict[str, Any] = {
        "peakRssBytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
    }
    if backend == "mps":
        try:
            usage["mpsAllocatedBytes"] = torch_module.mps.current_allocated_memory()
            usage["mpsDriverBytes"] = torch_module.mps.driver_allocated_memory()
        except Exception:
            pass
    return usage


def fail(request_id: str | None, code: str, started: float) -> None:
    emit(
        {
            "version": PROTOCOL,
            "id": request_id,
            "ok": False,
            "failureCode": code,
            "elapsedMs": int((time.monotonic() - started) * 1000),
        }
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-path", required=True)
    parser.add_argument("--model-revision", required=True)
    args = parser.parse_args()

    load_started = time.monotonic()
    try:
        import numpy as np
        import torch
        from sentence_transformers import SentenceTransformer
        from sentence_transformers import __version__ as sentence_transformers_version

        backend = "mps" if torch.backends.mps.is_available() else "cpu"
        model = SentenceTransformer(
            args.model_path,
            trust_remote_code=True,
            device=backend,
            model_kwargs={"dtype": torch.bfloat16},
        )
    except Exception:
        # Do not print exception text: paths and third-party messages are not a research receipt.
        emit({"version": PROTOCOL, "type": "ready", "ok": False, "failureCode": "model_load_failed"})
        return 1

    emit(
        {
            "version": PROTOCOL,
            "type": "ready",
            "ok": True,
            "modelId": MODEL_ID,
            "modelRevision": args.model_revision,
            "encodingMode": "document",
            "dimension": DIMENSION,
            "backend": backend,
            "dtype": str(torch.bfloat16),
            "torchVersion": torch.__version__,
            "sentenceTransformersVersion": sentence_transformers_version,
            "coldStartMs": int((time.monotonic() - load_started) * 1000),
            **resources(torch, backend),
        }
    )

    for line in sys.stdin:
        started = time.monotonic()
        request_id: str | None = None
        try:
            request = json.loads(line)
            request_id = request.get("id")
            texts = request.get("texts")
            if (
                request.get("encodingMode") != "document"
                or not isinstance(request_id, str)
                or not isinstance(texts, list)
                or not 1 <= len(texts) <= MAX_TEXTS_PER_REQUEST
                or any(not isinstance(text, str) or not text or len(text) > MAX_TEXT_CHARS for text in texts)
            ):
                fail(request_id, "invalid_request", started)
                continue
            vectors = model.encode_document(
                texts,
                batch_size=min(32, len(texts)),
                normalize_embeddings=True,
                truncate_dim=DIMENSION,
                convert_to_numpy=True,
                show_progress_bar=False,
            )
            norms = np.linalg.norm(vectors, axis=1, keepdims=True)
            if vectors.shape != (len(texts), DIMENSION) or not np.isfinite(vectors).all() or (norms <= 0).any():
                fail(request_id, "invalid_embedding_output", started)
                continue
            # The explicit second normalization is part of the local profile contract, even though
            # SentenceTransformer already normalizes.  It makes the persisted pgvector invariant
            # exact enough for cosine distance and rejects accidental model behaviour drift.
            vectors = vectors / norms
            emit(
                {
                    "version": PROTOCOL,
                    "id": request_id,
                    "ok": True,
                    "backend": backend,
                    "dimension": DIMENSION,
                    "values": vectors.astype("float32").tolist(),
                    "elapsedMs": int((time.monotonic() - started) * 1000),
                    **resources(torch, backend),
                }
            )
        except Exception:
            fail(request_id, "runtime_failed", started)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
