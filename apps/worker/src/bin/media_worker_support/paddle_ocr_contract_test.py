"""Pinned contract checks for the PaddleOCR 3.7 prediction JSON consumed by the bridge."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


BRIDGE_PATH = Path(__file__).with_name("paddle_ocr.py")
SPEC = importlib.util.spec_from_file_location("linggan_paddle_ocr", BRIDGE_PATH)
assert SPEC is not None and SPEC.loader is not None
BRIDGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BRIDGE)

# Captured from PaddleOCR 3.7.0 `result[0].json` using the PP-OCRv4 mobile Chinese pipeline.
# The fixture is intentionally small: it freezes the external `res.rec_*` / `res.dt_polys`
# field contract without adding a copyrighted source image or a full OCR corpus to the repository.
CAPTURED_PREDICTION = {
    "res": {
        "rec_texts": ["不要带 A 娃", "吊在一棵树上！"],
        "rec_scores": [0.982, 0.976],
        "dt_polys": [
            [[130, 985], [819, 985], [819, 1073], [130, 1073]],
            [[151, 1092], [908, 1092], [908, 1180], [151, 1180]],
        ],
    }
}


class PaddleOcrPredictionContractTest(unittest.TestCase):
    def test_captured_prediction_is_normalized_without_rewriting_text(self) -> None:
        lines = BRIDGE.parse_prediction(CAPTURED_PREDICTION, 1080, 1440)
        self.assertEqual([line["text"] for line in lines], ["不要带 A 娃", "吊在一棵树上！"])
        self.assertEqual(lines[0]["confidence"], 0.982)
        self.assertEqual(lines[0]["bboxNorm"], [0.12037, 0.684028, 0.758333, 0.745139])

    def test_missing_external_fields_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "result_fields_missing"):
            BRIDGE.parse_prediction({"res": {"rec_texts": []}}, 1080, 1440)


if __name__ == "__main__":
    unittest.main()
