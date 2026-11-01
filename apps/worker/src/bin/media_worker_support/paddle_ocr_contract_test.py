"""Pinned contract checks for the PaddleOCR 3.7 prediction JSON consumed by the bridge."""

from __future__ import annotations

import importlib.util
import tempfile
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
    def test_worker_invocation_keeps_existing_defaults(self) -> None:
        args = BRIDGE.parse_arguments(["local.jpg"])
        self.assertEqual((args.image_scale, args.text_det_box_thresh), (1, 0.6))

    def test_enlarged_input_preserves_color_and_source_coordinates(self) -> None:
        from PIL import Image

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.png"
            Image.new("RGB", (20, 10), (255, 0, 0)).save(path)
            original = path.read_bytes()
            pixels, width, height = BRIDGE.prediction_input(path, 2)
            self.assertEqual((width, height), (40, 20))
            self.assertEqual(pixels[0, 0].tolist(), [0, 0, 255])
            payload = {"res": {
                "rec_texts": ["原文"], "rec_scores": [0.9],
                "dt_polys": [[[10, 5], [30, 5], [30, 15], [10, 15]]],
            }}
            self.assertEqual(BRIDGE.parse_prediction(payload, width, height)[0]["bboxNorm"],
                             [0.25, 0.25, 0.75, 0.75])
            self.assertEqual(path.read_bytes(), original)

    def test_enlargement_stops_at_existing_detection_ceiling(self) -> None:
        from PIL import Image

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "wide.png"
            Image.new("RGB", (3000, 30)).save(path)
            _, width, height = BRIDGE.prediction_input(path, 2)
            self.assertEqual((width, height), (4000, 40))

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
