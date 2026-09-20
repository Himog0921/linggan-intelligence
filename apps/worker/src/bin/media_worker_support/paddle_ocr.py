#!/usr/bin/env python3
"""Bounded PaddleOCR bridge for one already-local image.

The Rust worker owns storage, leases, and database writes.  This process only reads the supplied
image and emits a compact schema-checked JSON document on stdout; it never receives database or
provider credentials and never contacts a platform.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def paddle_cache_dir() -> Path:
    configured_support_dir = os.environ.get("LINGGAN_SUPPORT_DIR")
    support_dir = (
        Path(configured_support_dir).expanduser()
        if configured_support_dir
        else Path.home() / "Library/Application Support/Linggan Intelligence"
    )
    return support_dir / "paddle-ocr" / "cache"


def fail(code: str) -> None:
    print(json.dumps({"schemaVersion": 1, "ok": False, "error": code}), flush=True)
    raise SystemExit(2)


def rectangle(points: object, width: int, height: int) -> tuple[float, float, float, float] | None:
    if not isinstance(points, list) or len(points) < 4:
        return None
    coordinates: list[tuple[float, float]] = []
    for point in points:
        if not isinstance(point, list) or len(point) != 2:
            return None
        try:
            coordinates.append((float(point[0]), float(point[1])))
        except (TypeError, ValueError):
            return None
    left = max(0.0, min(x for x, _ in coordinates) / width)
    top = max(0.0, min(y for _, y in coordinates) / height)
    right = min(1.0, max(x for x, _ in coordinates) / width)
    bottom = min(1.0, max(y for _, y in coordinates) / height)
    if right < left or bottom < top:
        return None
    return (left, top, right, bottom)


def parse_prediction(payload: object, width: int, height: int) -> list[dict[str, object]]:
    if not isinstance(payload, dict):
        raise ValueError("result_payload_invalid")
    data = payload.get("res")
    if not isinstance(data, dict):
        raise ValueError("result_payload_invalid")
    required = ("rec_texts", "rec_scores", "dt_polys")
    if any(key not in data for key in required):
        raise ValueError("result_fields_missing")
    texts = data["rec_texts"]
    scores = data["rec_scores"]
    polygons = data["dt_polys"]
    if not isinstance(texts, list) or not isinstance(scores, list) or not isinstance(polygons, list):
        raise ValueError("result_shape_invalid")
    if len(texts) != len(scores) or len(texts) != len(polygons):
        raise ValueError("result_line_count_mismatch")
    lines = []
    for index, text in enumerate(texts):
        if not isinstance(text, str):
            raise ValueError("result_line_text_invalid")
        if not text.strip():
            continue
        try:
            score = float(scores[index])
        except (TypeError, ValueError) as error:
            raise ValueError("result_line_score_invalid") from error
        box = rectangle(polygons[index], width, height)
        if box is None or not 0 <= score <= 1:
            raise ValueError("result_line_geometry_invalid")
        lines.append({
            "text": text.strip(),
            "confidence": round(score, 6),
            "bboxNorm": [round(value, 6) for value in box],
        })
    return lines


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage")
    image_path = Path(sys.argv[1])
    if not image_path.is_file():
        fail("input_missing")
    try:
        from PIL import Image
        width, height = Image.open(image_path).size
    except Exception:
        fail("image_unreadable")
    if width <= 0 or height <= 0:
        fail("image_dimensions_invalid")

    os.environ.setdefault("PADDLE_PDX_CACHE_HOME", str(paddle_cache_dir()))
    try:
        import paddle
        import paddleocr
        from paddleocr import PaddleOCR
    except Exception:
        fail("paddle_runtime_unavailable")
    engine = PaddleOCR(
        lang="ch",
        ocr_version="PP-OCRv4",
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=False,
    )
    result = list(engine.predict(str(image_path)))
    if len(result) != 1:
        fail("result_count_invalid")
    payload = result[0].json
    if isinstance(payload, str):
        payload = json.loads(payload)
    try:
        lines = parse_prediction(payload, width, height)
    except ValueError as error:
        fail(str(error))
    print(json.dumps({
        "schemaVersion": 1,
        "ok": True,
        "engine": "paddleocr",
        "engineVersion": f"paddleocr-{paddleocr.__version__};paddle-{paddle.__version__};PP-OCRv4-mobile",
        "imageWidth": width,
        "imageHeight": height,
        "lines": lines,
    }, ensure_ascii=False, separators=(",", ":")), flush=True)


if __name__ == "__main__":
    main()
