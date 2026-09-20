#!/usr/bin/env python3
"""Bounded PaddleOCR bridge for one already-local image.

The Rust worker owns storage, leases, and database writes.  This process only reads the supplied
image and emits a compact schema-checked JSON document on stdout; it never receives database or
provider credentials and never contacts a platform.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import paddle
import paddleocr
from PIL import Image
from paddleocr import PaddleOCR


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


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage")
    image_path = Path(sys.argv[1])
    if not image_path.is_file():
        fail("input_missing")
    try:
        width, height = Image.open(image_path).size
    except Exception:
        fail("image_unreadable")
    if width <= 0 or height <= 0:
        fail("image_dimensions_invalid")

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
    data = payload.get("res", {}) if isinstance(payload, dict) else {}
    texts = data.get("rec_texts", [])
    scores = data.get("rec_scores", [])
    polygons = data.get("dt_polys", [])
    if not isinstance(texts, list) or not isinstance(scores, list) or not isinstance(polygons, list):
        fail("result_shape_invalid")
    lines = []
    for index, text in enumerate(texts):
        if not isinstance(text, str) or not text.strip() or index >= len(scores) or index >= len(polygons):
            continue
        try:
            score = float(scores[index])
        except (TypeError, ValueError):
            continue
        box = rectangle(polygons[index], width, height)
        if box is None or not 0 <= score <= 1:
            continue
        lines.append({
            "text": text.strip(),
            "confidence": round(score, 6),
            "bboxNorm": [round(value, 6) for value in box],
        })
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
