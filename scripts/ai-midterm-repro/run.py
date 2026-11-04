#!/usr/bin/env python3
"""Execute unchanged Rust decision functions with synthetic, in-memory dependencies."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "artifacts/private/ai-midterm-quality-002/repro"
OUT.mkdir(parents=True, exist_ok=True)
RUN = Path(tempfile.mkdtemp(prefix="run-", dir=OUT))
(RUN / "src").mkdir()
manifest = []


def extract(path, start, end):
    raw = (ROOT / path).read_bytes()
    begin = raw.index(start.encode())
    finish = raw.index(end.encode(), begin)
    snippet = raw[begin:finish]
    manifest.append({"path": path, "startLine": raw[:begin].count(b"\n") + 1,
                     "endLine": raw[:finish].count(b"\n"),
                     "sourceSha256": hashlib.sha256(raw).hexdigest(),
                     "extractedSha256": hashlib.sha256(snippet).hexdigest()})
    return snippet.decode()


topic = extract("crates/intelligence/src/topic_map_research_analysis/resolution.rs",
                "fn candidate_versions(", "/// These checks")
problem_path = "crates/intelligence/src/ai_foundation/comment_problem.rs"
identity = extract(problem_path, "fn problem_identity(", "async fn proposals_in(")
pair = extract(problem_path, "async fn pair_support_in(", "pub(crate) struct Packet")
types = extract("crates/intelligence/src/ai_foundation/comment_problem/contract.rs",
                "#[derive(Deserialize, Serialize)]", "#[derive(Deserialize, Serialize)]\n#[serde(rename_all = \"camelCase\", deny_unknown_fields)]\npub(super) struct Check")
hash_fn = extract("crates/intelligence/src/research_text.rs",
                  "pub fn content_hash(", "pub fn valid_text(")
prefix = ("use serde_json::{Value, json};\nuse std::collections::HashMap;\n"
          "use uuid::Uuid;\n"
          "mod research_text { use sha2::{Digest, Sha256};\n" + hash_fn + "}\n"
          "mod contract { use serde::{Deserialize, Serialize}; use uuid::Uuid;\n" + types + "}\n"
          "use contract::{Problem, Resolution};\n")
template = (Path(__file__).parent / "harness.rs").read_text()
(RUN / "src/main.rs").write_text(prefix + topic + identity + pair + template)
(RUN / "Cargo.toml").write_text('''[package]
name = "ai-midterm-repro"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = "=1.0.151"
sha2 = "=0.10.9"
uuid = { version = "=1.24.1", features = ["serde"] }
[profile.dev]
debug = 0
incremental = false
''')
env = dict(os.environ)
env["CARGO_TARGET_DIR"] = str(RUN / "target")
command = ["cargo", "run", "--offline", "--quiet", "--manifest-path", str(RUN / "Cargo.toml")]
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
(RUN / "stdout.json").write_text(result.stdout)
(RUN / "stderr.log").write_text(result.stderr)
receipt = {"startedAt": started, "finishedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
           "baseHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
           "command": command, "exitCode": result.returncode, "extractions": manifest,
           "harnessSha256": hashlib.sha256((RUN / "src/main.rs").read_bytes()).hexdigest(),
           "evidenceLevel": "unchanged Rust functions; synthetic in-memory DB helper substitutes",
           "notProven": ["SQL filtering and transactions", "full queue/dispatch", "provider cost", "semantic equivalence accuracy", "production state"]}
(RUN / "receipt.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")
(OUT / "latest.json").write_text(json.dumps({"runDirectory": str(RUN), "exitCode": result.returncode}, indent=2) + "\n")
print(json.dumps({"runDirectory": str(RUN), "exitCode": result.returncode}, ensure_ascii=False))
if result.returncode:
    print(result.stderr)
else:
    print(result.stdout)
raise SystemExit(result.returncode)
