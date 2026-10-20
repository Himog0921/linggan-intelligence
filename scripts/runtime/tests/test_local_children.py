"""Run the exact foreground cleanup/EXIT trap with four owned dummy children and a DB double."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import unittest
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]

DUMMY = r'''#!/bin/sh
role="$1"
finish() {
  case "$role" in
    patrol) printf 'pid=%s\nstate=drained\n' "$$" > "$LINGGAN_WORKER_DRAIN_ACK_PATH" ;;
    ai)
      if [ "$CHILD_PROOF_MODE" = failed_ai ]; then
        printf '{"state":"failed","processId":%s,"reason":"synthetic_failure"}\n' "$$" > "$LINGGAN_AI_DRAIN_ACK_PATH"
        exit 1
      fi
      printf '{"state":"drained","processId":%s,"receipt":{"instanceRef":"%s","generation":"1","processId":%s,"sourceRevision":"unknown"}}\n' "$$" "$CHILD_PROOF_INSTANCE" "$$" > "$LINGGAN_AI_DRAIN_ACK_PATH" ;;
    *) printf stopped > "$CHILD_PROOF_ROOT/$role.stopped" ;;
  esac
  exit 0
}
trap finish TERM
touch "$CHILD_PROOF_ROOT/$role.ready"
while :; do sleep 0.03; done
'''

PRELUDE = r'''set -euo pipefail
runtime_drain_dir="$CHILD_PROOF_ROOT/drain"
mkdir "$runtime_drain_dir"
export LINGGAN_WORKER_DRAIN_ACK_PATH="$runtime_drain_dir/patrol"
export LINGGAN_AI_DRAIN_ACK_PATH="$runtime_drain_dir/ai"
runtime_ai_guard="$CHILD_PROOF_GUARD"
runtime_drain_grace=5
/bin/sh "$CHILD_PROOF_ROOT/dummy.sh" patrol >/dev/null 2>&1 &
scheduler_pid=$!
/bin/sh "$CHILD_PROOF_ROOT/dummy.sh" ai >/dev/null 2>&1 &
ai_worker_pid=$!
/bin/sh "$CHILD_PROOF_ROOT/dummy.sh" media >/dev/null 2>&1 &
media_worker_pid=$!
/bin/sh "$CHILD_PROOF_ROOT/dummy.sh" api >/dev/null 2>&1 &
api_pid=$!
export CHILD_PROOF_AI_PID="$ai_worker_pid"
python3 -B - <<'PY'
import json,os,pathlib
row={"instanceRef":os.environ["CHILD_PROOF_INSTANCE"],"generation":"1","processId":int(os.environ["CHILD_PROOF_AI_PID"]),"sourceRevision":"unknown","state":"drained"}
snapshot={"contract":"ai-drain-snapshot.v1","databaseKey":"synthetic:owned:public","runningInvocations":int(os.environ["CHILD_PROOF_MODE"]=="inflight"),"runningItems":0,"sessionLockHeld":False,"executor":row}
pathlib.Path(os.environ["CHILD_PROOF_ROOT"],"snapshot.json").write_text(json.dumps(snapshot))
PY
while [ ! -f "$CHILD_PROOF_ROOT/patrol.ready" ] || [ ! -f "$CHILD_PROOF_ROOT/ai.ready" ] || [ ! -f "$CHILD_PROOF_ROOT/media.ready" ] || [ ! -f "$CHILD_PROOF_ROOT/api.ready" ]; do sleep 0.02; done
psql_in_container() {
  case "$*" in
    *to_regclass*) printf '1,1\n' ;;
    *"SELECT jsonb_build_object"*) cat "$CHILD_PROOF_ROOT/snapshot.json" ;;
    *) return 97 ;;
  esac
}
'''


class LocalChildren(unittest.TestCase):
    def run_mode(self, mode):
        root = ROOT / "artifacts/private/ai-gen1-pr1b" / ("install-proof." + str(uuid.uuid4()))
        root.mkdir(mode=0o700)
        child = None
        try:
            source = (ROOT / "scripts/local-runtime.sh").read_text()
            begin = source.index("    cleanup_runtime_children() {")
            end = source.index("    trap 'runtime_exit_code=", begin)
            trap = source[end:].splitlines()[0]
            (root / "dummy.sh").write_text(DUMMY)
            driver = root / "driver.sh"; driver.write_text(PRELUDE + source[begin:end] + trap + "\ntrue\n")
            env = {"PATH": str(Path(sys.executable).parent) + ":/usr/bin:/bin", "CHILD_PROOF_ROOT": str(root),
                   "CHILD_PROOF_MODE": mode, "CHILD_PROOF_INSTANCE": str(uuid.uuid4()),
                   "CHILD_PROOF_GUARD": str(ROOT / "scripts/runtime/ai_guard.py")}
            child = subprocess.Popen(["/bin/bash", str(driver)], env=env, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, start_new_session=True)
            stdout, stderr = child.communicate(timeout=8)
            self.assertEqual(child.returncode, 0 if mode == "success" else 1, stdout + stderr)
            if mode == "success":
                self.assertTrue((root / "api.stopped").exists())
                self.assertTrue((root / "media.stopped").exists())
                self.assertFalse((root / "drain").exists())
            else:
                self.assertFalse((root / "api.stopped").exists())
                self.assertFalse((root / "media.stopped").exists())
                self.assertTrue((root / "drain/ai").is_file())
        finally:
            if child is not None:
                try: os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError: pass
                child.wait()
            shutil.rmtree(root)

    def test_success_stops_only_owned_children_after_both_receipts_and_database_proof(self):
        self.run_mode("success")

    def test_failed_ai_receipt_keeps_other_children_and_produces_a_failure_exit(self):
        self.run_mode("failed_ai")

    def test_unsettled_database_cannot_be_hidden_by_successful_child_receipts(self):
        self.run_mode("inflight")


if __name__ == "__main__":
    unittest.main()
