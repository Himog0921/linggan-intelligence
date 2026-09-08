#!/usr/bin/env python3
"""Offline CI V1 evaluation. Only local JSONL inputs; no model invocation or network access."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import sys


MODES = ("B0", "B1", "B2")
LABELS = ("need", "solution", "story", "quote")
GROUPS = ("resonance", "conflict")
SCHEMA_VERSION = "comment-intelligence-evaluation.v1"


class InvalidInput(ValueError):
    """Messages contain validation codes and line numbers, never input comment text."""


def require(condition, code):
    if not condition:
        raise InvalidInput(code)


def identifier(value):
    if not isinstance(value, str) or not 0 < len(value) <= 200 or any(ord(c) < 32 for c in value):
        return False
    try:
        value.encode("utf-8")
        return True
    except UnicodeError:
        return False


def string_set(value, allowed=None):
    return isinstance(value, list) and all(identifier(v) for v in value) and len(value) == len(set(value)) and (allowed is None or set(value) <= set(allowed))


def sha256(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "DUPLICATE_JSON_KEY")
        result[key] = value
    return result


def read_jsonl(path):
    rows = []
    try:
        with path.open(encoding="utf-8") as stream:
            for line_no, line in enumerate(stream, 1):
                if not line.strip():
                    continue
                require(len(line) <= 2_000_000, f"LINE_TOO_LARGE:{line_no}")
                try:
                    row = json.loads(line, object_pairs_hook=unique_object,
                                     parse_constant=lambda _: (_ for _ in ()).throw(InvalidInput("NONFINITE_JSON_NUMBER")))
                except (json.JSONDecodeError, InvalidInput) as error:
                    raise InvalidInput(f"INVALID_JSON:{line_no}") from error
                require(isinstance(row, dict), f"ROW_NOT_OBJECT:{line_no}")
                rows.append(row)
                require(len(rows) <= 30_000, "EVALUATION_ROW_LIMIT")
    except (OSError, UnicodeError) as error:
        raise InvalidInput("INPUT_UNREADABLE") from error
    return rows


def validate_gold(rows):
    require(bool(rows), "EMPTY_GOLD")
    gold = {}
    work_splits = {}
    reviews = {}
    for row in rows:
        require(all(key in row for key in ("sourceRef", "workRef", "text", "labels", "problemKey", "split", "isSynthetic")), "GOLD_REQUIRED_FIELD")
        require(identifier(row["sourceRef"]) and identifier(row["workRef"]), "GOLD_ID_INVALID")
        require(row["sourceRef"] not in gold, "DUPLICATE_GOLD_SOURCE")
        require(isinstance(row["text"], str) and len(row["text"]) <= 100_000, "GOLD_TEXT_INVALID")
        try:
            row["text"].encode("utf-8")
        except UnicodeError as error:
            raise InvalidInput("GOLD_UNICODE_INVALID") from error
        require(string_set(row["labels"], LABELS), "GOLD_LABELS_INVALID")
        require(row["problemKey"] is None or identifier(row["problemKey"]), "GOLD_PROBLEM_KEY_INVALID")
        require(row["split"] in ("calibration", "holdout"), "GOLD_SPLIT_INVALID")
        require(type(row["isSynthetic"]) is bool, "GOLD_SYNTHETIC_DECLARATION_REQUIRED")
        require(row["workRef"] not in work_splits or work_splits[row["workRef"]] == row["split"], "WORK_SPLIT_LEAKAGE")
        work_splits[row["workRef"]] = row["split"]
        gold[row["sourceRef"]] = row
        require(isinstance(row.get("groupReviews", []), list), "GROUP_REVIEWS_INVALID")
        for review in row.get("groupReviews", []):
            require(isinstance(review, dict), "GROUP_REVIEW_INVALID")
            require(identifier(review.get("observationRef")) and review.get("kind") in GROUPS, "GROUP_REVIEW_ID_INVALID")
            require(type(review.get("accepted")) is bool and review.get("reviewer") == "human", "INDEPENDENT_GROUP_REVIEW_REQUIRED")
            require(string_set(review.get("sourceRefs")) and bool(review["sourceRefs"]), "GROUP_REVIEW_EVIDENCE_INVALID")
            key = review["observationRef"]
            require(key not in reviews or reviews[key] == review, "CONFLICTING_GROUP_REVIEW")
            reviews[key] = review
    for review in reviews.values():
        require(all(source in gold for source in review["sourceRefs"]), "GROUP_REVIEW_UNKNOWN_SOURCE")
        require(len({gold[source]["split"] for source in review["sourceRefs"]}) == 1, "GROUP_REVIEW_SPLIT_LEAKAGE")
        require(len({gold[source]["isSynthetic"] for source in review["sourceRefs"]}) == 1, "GROUP_REVIEW_POPULATION_MIX")
    return gold, reviews


def validate_predictions(rows, gold):
    predictions = {mode: {} for mode in MODES}
    observations = {mode: {} for mode in MODES}
    for row in rows:
        require(all(key in row for key in ("sourceRef", "labels", "problemRef", "sourceSha256", "evidence", "contextMode", "modelVersion", "ruleVersion")), "PREDICTION_REQUIRED_FIELD")
        mode = row["contextMode"]
        require(mode in MODES, "PREDICTION_MODE_INVALID")
        source = row["sourceRef"]
        require(identifier(source) and source in gold, "PREDICTION_UNKNOWN_SOURCE")
        require(source not in predictions[mode], "DUPLICATE_MODE_PREDICTION")
        require(string_set(row["labels"], LABELS), "PREDICTION_LABELS_INVALID")
        require(row["problemRef"] is None or identifier(row["problemRef"]), "PREDICTION_PROBLEM_REF_INVALID")
        require(identifier(row["modelVersion"]) and identifier(row["ruleVersion"]), "PREDICTION_VERSION_INVALID")
        require(isinstance(row["sourceSha256"], str), "PREDICTION_HASH_INVALID")
        require(isinstance(row["evidence"], list), "PREDICTION_EVIDENCE_INVALID")
        for field in ("latencyMs", "inputTokens", "outputTokens"):
            value = row.get(field)
            require(value is None or (type(value) in (int, float) and math.isfinite(value) and value >= 0), "PREDICTION_USAGE_INVALID")
        predictions[mode][source] = row
        require(isinstance(row.get("observations", []), list), "OBSERVATIONS_INVALID")
        for item in row.get("observations", []):
            require(isinstance(item, dict) and identifier(item.get("observationRef")) and item.get("kind") in GROUPS, "OBSERVATION_INVALID")
            require(string_set(item.get("sourceRefs")) and bool(item["sourceRefs"]), "OBSERVATION_EVIDENCE_INVALID")
            require(all(source in gold for source in item["sourceRefs"]), "OBSERVATION_UNKNOWN_SOURCE")
            require(len({gold[source]["split"] for source in item["sourceRefs"]}) == 1, "OBSERVATION_SPLIT_LEAKAGE")
            require(len({gold[source]["isSynthetic"] for source in item["sourceRefs"]}) == 1, "OBSERVATION_POPULATION_MIX")
            key = item["observationRef"]
            require(key not in observations[mode] or observations[mode][key] == item, "CONFLICTING_OBSERVATION")
            observations[mode][key] = item
    return predictions, observations


def divide(numerator, denominator):
    return round(numerator / denominator, 6) if denominator else None


def scores(tp, fp, fn):
    return {"truePositive": tp, "falsePositive": fp, "falseNegative": fn,
            "precision": divide(tp, tp + fp), "recall": divide(tp, tp + fn),
            "f1": divide(2 * tp, 2 * tp + fp + fn), "goldPositive": tp + fn,
            "predictedPositive": tp + fp}


def evidence_metrics(sources, predicted, gold):
    total = valid = hash_errors = missing_evidence = 0
    invalid = []
    for source in sources:
        if source not in predicted:
            continue
        row = predicted[source]
        if row["sourceSha256"] != sha256(gold[source]["text"]):
            hash_errors += 1
        if (row["labels"] or row["problemRef"] is not None) and not row["evidence"]:
            missing_evidence += 1
        for evidence in row["evidence"]:
            total += 1
            correct = False
            if isinstance(evidence, dict):
                ref = evidence.get("sourceRef")
                start, end = evidence.get("startChar"), evidence.get("endChar")
                quote = evidence.get("quote")
                if isinstance(ref, str) and ref in gold and type(start) is int and type(end) is int and isinstance(quote, str):
                    text = gold[ref]["text"]
                    correct = 0 <= start < end <= len(text) and text[start:end] == quote
            if correct:
                valid += 1
            elif len(invalid) < 100:
                invalid.append({"sourceRef": source, "code": "QUOTE_OR_COORDINATES_INVALID"})
    return {"total": total, "valid": valid, "validRate": divide(valid, total),
            "sourceHashErrors": hash_errors, "claimsWithoutEvidence": missing_evidence,
            "invalidExamples": invalid, "invalidExamplesTruncated": total - valid > len(invalid),
            "passed": total > 0 and valid == total and hash_errors == 0 and missing_evidence == 0}


def choose_two(count):
    return count * (count - 1) // 2


def grouping_metrics(sources, predicted, gold):
    clusters = defaultdict(list)
    gold_clusters = Counter()
    eligible = unmerged = 0
    for source in sources:
        key = gold[source]["problemKey"]
        problem = predicted.get(source, {}).get("problemRef")
        if key is not None:
            eligible += 1
            gold_clusters[key] += 1
            unmerged += problem is None
        if problem is not None:
            clusters[problem].append(key)
    predicted_pairs = sum(choose_two(len(members)) for members in clusters.values())
    true_pairs = sum(choose_two(count) for members in clusters.values() for key, count in Counter(members).items() if key is not None)
    gold_pairs = sum(choose_two(count) for count in gold_clusters.values())
    false_pairs = predicted_pairs - true_pairs
    return {"predictedPairs": predicted_pairs, "correctPairs": true_pairs, "incorrectPairs": false_pairs,
            "goldPairs": gold_pairs, "mismergeRate": divide(false_pairs, predicted_pairs),
            "pairwisePrecision": divide(true_pairs, predicted_pairs), "pairwiseRecall": divide(true_pairs, gold_pairs),
            "eligibleComments": eligible, "unmergedComments": unmerged, "unmergedRate": divide(unmerged, eligible),
            "interpretation": "Pairwise error among assigned pairs; never a count of distinct users."}


def group_metrics(sources, observations, reviews):
    scope = set(sources)
    result = {}
    for kind in GROUPS:
        predicted = {key: value for key, value in observations.items() if value["kind"] == kind and set(value["sourceRefs"]) <= scope}
        expected = {key: value for key, value in reviews.items() if value["kind"] == kind and set(value["sourceRefs"]) <= scope}
        tp = fp = unreviewed = mismatched = 0
        for key, item in predicted.items():
            review = expected.get(key)
            if review is None:
                unreviewed += 1
            elif set(review["sourceRefs"]) != set(item["sourceRefs"]):
                mismatched += 1
            elif review["accepted"]:
                tp += 1
            else:
                fp += 1
        fn = sum(item["accepted"] and key not in predicted for key, item in expected.items())
        result[kind] = {**scores(tp, fp, fn), "unreviewed": unreviewed,
                        "evidenceSetMismatch": mismatched, "predictedObservations": len(predicted),
                        "reviewStatus": "REVIEWED" if tp + fp > 0 and unreviewed == 0 and mismatched == 0 else "INSUFFICIENT_EVIDENCE"}
    return result


def usage_metrics(sources, predicted):
    result = {}
    for field in ("latencyMs", "inputTokens", "outputTokens"):
        values = [predicted[source].get(field) for source in sources if source in predicted]
        known = sorted(value for value in values if value is not None)
        result[field] = {"knownSamples": len(known), "unknownSamples": len(values) - len(known),
                         "sumKnown": sum(known) if known else None,
                         "meanKnown": divide(sum(known), len(known))}
        if field == "latencyMs":
            result[field]["p95Known"] = known[math.ceil(len(known) * .95) - 1] if known else None
    return result


def mode_metrics(sources, predicted, gold, observations, reviews):
    labels = {}
    for label in LABELS:
        tp = fp = fn = 0
        for source in sources:
            expected = label in gold[source]["labels"]
            actual = label in predicted.get(source, {}).get("labels", [])
            tp += expected and actual
            fp += not expected and actual
            fn += expected and not actual
        labels[label] = scores(tp, fp, fn)
    return {"samples": len(sources), "predictions": sum(source in predicted for source in sources),
            "missingPredictions": sum(source not in predicted for source in sources), "labels": labels,
            "evidence": evidence_metrics(sources, predicted, gold), "grouping": grouping_metrics(sources, predicted, gold),
            "groupObservations": group_metrics(sources, observations, reviews), "usage": usage_metrics(sources, predicted)}


def evaluate(gold_rows, prediction_rows):
    gold, reviews = validate_gold(gold_rows)
    predictions, observations = validate_predictions(prediction_rows, gold)
    real = [source for source, row in gold.items() if not row["isSynthetic"]]
    synthetic = [source for source, row in gold.items() if row["isSynthetic"]]
    versions = {}
    for mode in MODES:
        versions[mode] = sorted({(row["modelVersion"], row["ruleVersion"]) for row in predictions[mode].values()})
    comparable = all(len(versions[mode]) == 1 and set(predictions[mode]) == set(gold) for mode in MODES) and len({tuple(versions[mode]) for mode in MODES}) == 1
    metrics = {}
    for population, candidates in (("real", real), ("synthetic", synthetic)):
        metrics[population] = {}
        for split in ("calibration", "holdout"):
            sources = [source for source in candidates if gold[source]["split"] == split]
            metrics[population][split] = {mode: mode_metrics(sources, predictions[mode], gold, observations[mode], reviews) for mode in MODES}
    real_works = {gold[source]["workRef"] for source in real}
    blockers = []
    if not 200 <= len(real) <= 300:
        blockers.append("REAL_SAMPLE_OUTSIDE_PLANNED_200_TO_300")
    if len(real_works) < 20:
        blockers.append("REAL_WORKS_BELOW_20")
    if len(synthetic) > 60:
        blockers.append("SYNTHETIC_SAMPLES_ABOVE_60")
    if any(gold[source].get("annotationSource") != "human" for source in real):
        blockers.append("HUMAN_GOLD_DECLARATION_MISSING")
    if not all(any(gold[source]["split"] == split for source in real) for split in ("calibration", "holdout")):
        blockers.append("REAL_CALIBRATION_OR_HOLDOUT_MISSING")
    if not comparable:
        blockers.append("B0_B1_B2_INPUT_OR_VERSION_NOT_COMPARABLE")
    holdout = metrics["real"]["holdout"]["B2"]
    for label in ("need", "solution", "story"):
        precision = holdout["labels"][label]["precision"]
        if precision is None or precision < .85:
            blockers.append(f"CORE_PRECISION_NOT_MET:{label}")
    merge = holdout["grouping"]
    if merge["mismergeRate"] is None or merge["mismergeRate"] > .05:
        blockers.append("MISMERGE_THRESHOLD_NOT_MET_OR_NO_ASSIGNED_PAIRS")
    if merge["unmergedRate"] is None or merge["unmergedRate"] >= 1:
        blockers.append("UNMERGED_COVERAGE_INSUFFICIENT")
    for kind in GROUPS:
        group = holdout["groupObservations"][kind]
        if group["precision"] is None or group["precision"] < .90 or group["reviewStatus"] != "REVIEWED":
            blockers.append(f"INDEPENDENT_GROUP_PRECISION_NOT_MET:{kind}")
    if not holdout["evidence"]["passed"] or holdout["missingPredictions"]:
        blockers.append("HOLDOUT_REFERENCE_OR_COVERAGE_CHECK_FAILED")
    # Synthetic adversarial evidence is independently reported and cannot be blended into
    # the real precision denominator to manufacture a release qualification.
    for split in ("calibration", "holdout"):
        for mode in MODES:
            evidence = metrics["synthetic"][split][mode]["evidence"]
            if evidence["total"] != evidence["valid"] or evidence["sourceHashErrors"] or evidence["claimsWithoutEvidence"]:
                blockers.append(f"SYNTHETIC_REFERENCE_CHECK_FAILED:{split}:{mode}")
    gain = {}
    for label in LABELS:
        values = {mode: metrics["real"]["holdout"][mode]["labels"][label]["f1"] for mode in MODES}
        gain[label] = {"f1ByMode": values, "B1MinusB0": None if values["B0"] is None or values["B1"] is None else round(values["B1"] - values["B0"], 6),
                       "B2MinusB1": None if values["B1"] is None or values["B2"] is None else round(values["B2"] - values["B1"], 6)}
    return {"schemaVersion": SCHEMA_VERSION, "status": "QUALIFIED_EVALUATION" if not blockers else "NOT_QUALIFIED",
            "qualifiedAutomaticDiscovery": not blockers, "isSynthetic": not bool(real),
            "containsSynthetic": bool(synthetic), "provenance": "mixed" if real and synthetic else "real_declared" if real else "synthetic",
            "samples": {"real": len(real), "realWorks": len(real_works), "synthetic": len(synthetic),
                        "calibration": sum(row["split"] == "calibration" for row in gold.values()), "holdout": sum(row["split"] == "holdout" for row in gold.values())},
            "comparison": {"comparable": comparable, "versions": versions, "sameSourceSet": all(set(predictions[mode]) == set(gold) for mode in MODES),
                           "observedQualityDifferences": gain, "interpretation": "No positive context/retrieval gain is assumed; differences are descriptive, not causal proof."},
            "metrics": metrics, "thresholds": {"corePrecision": .85, "groupObservationPrecision": .90, "mismergeRateMaximum": .05},
            "blockers": blockers, "limits": ["Input provenance and human review are declarations; this program cannot authenticate them.",
                                               "Passing this local quality evaluation does not authorize deployment, external processing or automatic publication.",
                                               "Small per-class/group denominators remain visible; a threshold is not a confidence interval."]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gold", required=True, type=Path)
    parser.add_argument("--predictions", required=True, type=Path)
    parser.add_argument("--output", type=Path, help="Optional report path; use a private or temporary location.")
    parser.add_argument("--require-qualified", action="store_true", help="Exit 1 when a valid evaluation remains NOT_QUALIFIED.")
    args = parser.parse_args(argv)
    try:
        input_hashes = {"gold": hashlib.sha256(args.gold.read_bytes()).hexdigest(), "predictions": hashlib.sha256(args.predictions.read_bytes()).hexdigest()}
        report = evaluate(read_jsonl(args.gold), read_jsonl(args.predictions))
        final_hashes = {"gold": hashlib.sha256(args.gold.read_bytes()).hexdigest(), "predictions": hashlib.sha256(args.predictions.read_bytes()).hexdigest()}
        require(input_hashes == final_hashes, "INPUT_CHANGED_DURING_EVALUATION")
        report["inputSha256"] = input_hashes
        content = json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
        if args.output:
            require(args.output.resolve() not in {args.gold.resolve(), args.predictions.resolve()}, "OUTPUT_MUST_NOT_OVERWRITE_INPUT")
            args.output.write_text(content, encoding="utf-8")
        else:
            sys.stdout.write(content)
        return 1 if args.require_qualified and not report["qualifiedAutomaticDiscovery"] else 0
    except (InvalidInput, OSError) as error:
        code = str(error) if isinstance(error, InvalidInput) else "LOCAL_FILE_ACCESS_FAILED"
        sys.stderr.write(json.dumps({"schemaVersion": SCHEMA_VERSION, "status": "INVALID_INPUT", "qualifiedAutomaticDiscovery": False, "error": code}) + "\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
