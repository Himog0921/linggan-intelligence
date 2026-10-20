#!/usr/bin/env python3
"""Derive an isolated review's typography registry from the project B standard."""

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/design/lids/typography.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review-root", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    args = parser.parse_args()
    path = args.review_root / "spec/registry.json"
    registry = json.loads(path.read_text())
    if registry.get("meta", {}).get("id") != "LINGGAN-DESIGN-SPEC-DRAFT-01":
        parser.error("destination is not the registered isolated review")
    standard = json.loads(SOURCE.read_text())
    if standard.get("status") != "accepted":
        parser.error("source typography is not accepted")
    previous = registry["visual"]["typographyApplication"]
    derived = {
        **previous,
        "standardId": standard["id"],
        "status": "accepted_B_specification_runtime_not_migrated",
        "date": standard["date"],
        "userDecision": standard["decision"]["quote"],
        "generatedFrom": {
            "path": "docs/design/lids/typography.json",
            "checkout": str(ROOT),
            "sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
            "editPolicy": "Generated mirror; edit the project source, then rerun sync-lids-typography.py.",
        },
        **{key: standard[key] for key in ["families", "roles", "roleLabels", "weights", "rules", "deliveryDecision"]},
    }
    if args.check:
        if previous != derived:
            print("Typography registry drift: regenerate from the project B standard.")
            return 1
    else:
        registry["visual"]["typographyApplication"] = derived
        registry["visual"]["typographyAuthority"] = "LIDS-TYPE-001-B is the sole adopted typography standard. typographyApplication is its generated isolated-review mirror; prior candidates are historical. Business runtime remains unmigrated."
        path.write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"result": "passed", "standard": standard["id"], "roles": len(standard["roles"]), "mode": "check" if args.check else "write", "runtimeChanged": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
