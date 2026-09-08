"""Four explicit synthetic comments exercise the offline evaluator, not real model quality."""

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "evaluate-comment-intelligence.py"
SPEC = importlib.util.spec_from_file_location("comment_evaluation", SCRIPT)
evaluation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(evaluation)


def fixture():
    gold = [
        {"sourceRef": "s1", "workRef": "w1", "text": "合成：哪里有帮助", "labels": ["need"], "problemKey": "time", "split": "calibration", "isSynthetic": True},
        {"sourceRef": "s2", "workRef": "w2", "text": "合成：时间不够怎么办", "labels": ["need"], "problemKey": "time", "split": "holdout", "isSynthetic": True},
        {"sourceRef": "s3", "workRef": "w3", "text": "合成：我试过拆成两步", "labels": ["solution", "story"], "problemKey": "method", "split": "holdout", "isSynthetic": True},
        {"sourceRef": "s4", "workRef": "w4", "text": "合成：先迈出一小步", "labels": ["quote"], "problemKey": None, "split": "holdout", "isSynthetic": True},
    ]
    predictions = []
    for mode in evaluation.MODES:
        for row in gold:
            predictions.append({"sourceRef": row["sourceRef"], "labels": row["labels"][:], "problemRef": row["problemKey"],
                                "sourceSha256": evaluation.sha256(row["text"]),
                                "evidence": [{"sourceRef": row["sourceRef"], "startChar": 0, "endChar": len(row["text"]), "quote": row["text"]}],
                                "contextMode": mode, "modelVersion": "synthetic-model.v1", "ruleVersion": "synthetic-rule.v1"})
    return gold, predictions


class EvaluationTests(unittest.TestCase):
    def test_perfect_synthetic_predictions_never_qualify_real_discovery(self):
        gold, predictions = fixture()
        report = evaluation.evaluate(gold, predictions)
        self.assertEqual(report["status"], "NOT_QUALIFIED")
        self.assertFalse(report["qualifiedAutomaticDiscovery"])
        self.assertTrue(report["isSynthetic"])
        self.assertTrue(report["comparison"]["comparable"])
        self.assertEqual(report["metrics"]["synthetic"]["holdout"]["B2"]["labels"]["need"]["precision"], 1)
        self.assertIsNone(report["metrics"]["real"]["holdout"]["B2"]["labels"]["need"]["precision"])
        self.assertIn("REAL_WORKS_BELOW_20", report["blockers"])

    def test_work_split_leakage_and_duplicate_predictions_are_rejected(self):
        gold, predictions = fixture()
        gold[1]["workRef"] = gold[0]["workRef"]
        with self.assertRaisesRegex(evaluation.InvalidInput, "WORK_SPLIT_LEAKAGE"):
            evaluation.evaluate(gold, predictions)
        gold, predictions = fixture()
        predictions.append(copy.deepcopy(predictions[0]))
        with self.assertRaisesRegex(evaluation.InvalidInput, "DUPLICATE_MODE_PREDICTION"):
            evaluation.evaluate(gold, predictions)

    def test_reference_failures_are_counted_without_copying_comment_text_into_report(self):
        gold, predictions = fixture()
        row = next(row for row in predictions if row["contextMode"] == "B2" and row["sourceRef"] == "s2")
        row["sourceSha256"] = "wrong"
        row["evidence"][0]["quote"] = "not a source quote"
        report = evaluation.evaluate(gold, predictions)
        metrics = report["metrics"]["synthetic"]["holdout"]["B2"]["evidence"]
        self.assertEqual(metrics["sourceHashErrors"], 1)
        self.assertEqual(metrics["validRate"], round(2 / 3, 6))
        self.assertFalse(metrics["passed"])
        serialized = json.dumps(report, ensure_ascii=False)
        self.assertNotIn(gold[1]["text"], serialized)
        self.assertNotIn("not a source quote", serialized)

    def test_pairwise_error_and_unmerged_rate_cannot_reward_rejecting_everything(self):
        gold_rows, predictions = fixture()
        gold, _ = evaluation.validate_gold(gold_rows)
        predicted, _ = evaluation.validate_predictions(predictions, gold)
        for row in predicted["B2"].values():
            row["problemRef"] = "one-wrong-cluster"
        merged = evaluation.grouping_metrics(list(gold), predicted["B2"], gold)
        self.assertEqual(merged["predictedPairs"], 6)
        self.assertEqual(merged["correctPairs"], 1)
        self.assertEqual(merged["mismergeRate"], round(5 / 6, 6))
        for row in predicted["B2"].values():
            row["problemRef"] = None
        rejected = evaluation.grouping_metrics(list(gold), predicted["B2"], gold)
        self.assertIsNone(rejected["mismergeRate"])
        self.assertEqual(rejected["unmergedRate"], 1)

    def test_mode_model_version_or_sample_changes_make_comparison_invalid(self):
        gold, predictions = fixture()
        predictions[-1]["modelVersion"] = "different-model"
        report = evaluation.evaluate(gold, predictions)
        self.assertFalse(report["comparison"]["comparable"])
        self.assertIn("B0_B1_B2_INPUT_OR_VERSION_NOT_COMPARABLE", report["blockers"])

    def test_group_observation_requires_independent_review_of_exact_evidence_set(self):
        gold, predictions = fixture()
        review = {"observationRef": "obs-1", "kind": "conflict", "accepted": False, "sourceRefs": ["s2", "s3"], "reviewer": "human"}
        gold[1]["groupReviews"] = [review]
        row = next(row for row in predictions if row["contextMode"] == "B2" and row["sourceRef"] == "s2")
        row["observations"] = [{"observationRef": "obs-1", "kind": "conflict", "sourceRefs": ["s2", "s3"]}]
        report = evaluation.evaluate(gold, predictions)
        groups = report["metrics"]["synthetic"]["holdout"]["B2"]["groupObservations"]
        self.assertEqual(groups["conflict"]["precision"], 0)
        self.assertEqual(groups["conflict"]["falsePositive"], 1)
        self.assertIsNone(groups["resonance"]["precision"])
        row["observations"][0]["sourceRefs"] = ["s2"]
        report = evaluation.evaluate(gold, predictions)
        mismatch = report["metrics"]["synthetic"]["holdout"]["B2"]["groupObservations"]["conflict"]
        self.assertEqual(mismatch["evidenceSetMismatch"], 1)
        self.assertEqual(mismatch["reviewStatus"], "INSUFFICIENT_EVIDENCE")

    def test_cli_exit_codes_hashes_and_input_overwrite_guard(self):
        gold, predictions = fixture()
        with tempfile.TemporaryDirectory(prefix="ci-evaluation-test-") as directory:
            gold_path = Path(directory) / "gold.jsonl"
            prediction_path = Path(directory) / "predictions.jsonl"
            output = Path(directory) / "report.json"
            gold_path.write_text("\n".join(json.dumps(row) for row in gold), encoding="utf-8")
            prediction_path.write_text("\n".join(json.dumps(row) for row in predictions), encoding="utf-8")
            command = [sys.executable, str(SCRIPT), "--gold", str(gold_path), "--predictions", str(prediction_path)]
            completed = subprocess.run(command + ["--output", str(output), "--require-qualified"], capture_output=True, text=True, check=False)
            self.assertEqual(completed.returncode, 1, completed.stderr)
            report = json.loads(output.read_text())
            self.assertEqual(len(report["inputSha256"]["gold"]), 64)
            completed = subprocess.run(command + ["--output", str(gold_path)], capture_output=True, text=True, check=False)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("OUTPUT_MUST_NOT_OVERWRITE_INPUT", completed.stderr)
            self.assertEqual(len(evaluation.read_jsonl(gold_path)), 4)

    def test_provenance_unicode_and_json_duplicate_keys_are_not_silently_repaired(self):
        gold, predictions = fixture()
        del gold[0]["isSynthetic"]
        with self.assertRaisesRegex(evaluation.InvalidInput, "GOLD_REQUIRED_FIELD"):
            evaluation.evaluate(gold, predictions)
        gold, predictions = fixture()
        gold[0]["text"] = "\ud800"
        with self.assertRaisesRegex(evaluation.InvalidInput, "GOLD_UNICODE_INVALID"):
            evaluation.evaluate(gold, predictions)
        with tempfile.TemporaryDirectory(prefix="ci-evaluation-test-") as directory:
            path = Path(directory) / "duplicate.jsonl"
            path.write_text('{"sourceRef":"one","sourceRef":"two"}\n')
            with self.assertRaisesRegex(evaluation.InvalidInput, "INVALID_JSON:1"):
                evaluation.read_jsonl(path)


if __name__ == "__main__":
    unittest.main()
