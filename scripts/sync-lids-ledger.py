#!/usr/bin/env python3
"""Generate or check the isolated review mirror of the accepted ledger rule."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/design/lids/ledger-layout.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review-root", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    raw = SOURCE.read_bytes()
    source = json.loads(raw)
    if source.get("status") != "accepted" or source.get("decision", {}).get("selected") != "L1" or source.get("decision", {}).get("rowHeight") != 44 or source.get("widths", {}).get("name") != 144:
        parser.error("ledger source is not the accepted L1 rule")
    path = args.review_root / "spec/registry.json"
    registry = json.loads(path.read_text())
    if registry.get("meta", {}).get("id") != "LINGGAN-DESIGN-SPEC-DRAFT-01":
        parser.error("destination is not the registered isolated review")
    mirror = {**source, "generatedFrom": {
        "path": "docs/design/lids/ledger-layout.json", "checkout": str(ROOT),
        "sha256": hashlib.sha256(raw).hexdigest(),
        "editPolicy": "Generated mirror; edit the project source and rerun sync-lids-ledger.py.",
    }}
    if args.check:
        if registry.get("visual", {}).get("ledgerApplication") != mirror:
            print("Ledger mirror drift: regenerate from the project accepted L1 source.")
            return 1
    else:
        registry["visual"]["ledgerApplication"] = mirror
        path.write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"result": "passed", "standard": source["id"], "mode": "check" if args.check else "write", "runtimeChanged": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
