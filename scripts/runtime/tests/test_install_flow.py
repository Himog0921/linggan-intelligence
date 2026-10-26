"""Execute copied release scripts against owned files and strict command doubles only."""
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import unittest
import uuid

sys.dont_write_bytecode = True
SOURCE_ROOT = Path(__file__).resolve().parents[3]
LABELS = ["com.linggan-intelligence." + part for part in ("local-runtime", "patrol-worker", "media-worker", "ai-executor")]


class InstallFlow(unittest.TestCase):
    def setUp(self):
        self.root = SOURCE_ROOT / "artifacts/private/ai-gen1-pr1b" / ("install-proof." + str(uuid.uuid4()))
        self.root.mkdir(mode=0o700)
        self.source = self.root / "source"; self.runtime = self.root / "runtime & source"
        self.support = self.root / "support"; self.agents = self.root / "agents"; self.bin = self.root / "bin"
        for folder in (self.source, self.runtime, self.support, self.agents, self.bin, self.root / "tmp", self.root / "zsh"):
            folder.mkdir()
        for base in (self.source, self.runtime):
            folder = base / "scripts/runtime"; folder.mkdir(parents=True)
            for name in ("install.sh", "sync.sh", "launch.sh", "drain.zsh", "plist.zsh", "ai_guard.py", "ai_snapshot.py"):
                shutil.copy2(SOURCE_ROOT / "scripts/runtime" / name, folder / name)
            for name in ("prepare-web.sh", "prepare-pi-adapter.sh"):
                file = folder / name; file.write_text("#!/bin/sh\nexit 0\n"); file.chmod(0o700)
            resource = base / "crates/contracts/src/ai_limits.json"; resource.parent.mkdir(parents=True)
            shutil.copy2(SOURCE_ROOT / "crates/contracts/src/ai_limits.json", resource)
            migrations = base / "database/migrations"; migrations.mkdir(parents=True)
            for name in ("0131_ai_gen1_provider", "0140_ai_gen1_writing"): (migrations / (name + ".sql")).write_text("-- synthetic migration name only\n")
            (base / ".env").write_text("POSTGRES_USER=linggan_dev_admin\nPOSTGRES_PASSWORD=SYNTHETIC-NOT-A-SHARED-CREDENTIAL\nPOSTGRES_DB=synthetic_owned\nPOSTGRES_PORT=54321\nLINGGAN_LOCAL_PORT=3001\n")
            worker = base / "apps/worker/src/shutdown.rs"; worker.parent.mkdir(parents=True)
            worker.write_text("// synthetic marker: write_drain_ack\n")
        fixture = SOURCE_ROOT / "scripts/runtime/tests/fake_commands.py"
        for command in ("git", "launchctl", "docker", "cargo", "plutil", "pgrep", "lsof", "sleep"):
            file = self.bin / command; shutil.copy2(fixture, file); file.chmod(0o700)
        child = subprocess.Popen(["/usr/bin/true"]); pid = child.pid; child.wait()
        row = {"instanceRef": str(uuid.uuid4()), "generation": "7", "processId": pid, "sourceRevision": "a" * 40, "state": "running"}
        self.state = {"source": str(self.source), "runtime": str(self.runtime), "support": str(self.support),
                      "head": "a" * 40, "target": "b" * 40, "trace": str(self.root / "trace.jsonl"), "pid": pid,
                      "password": "SYNTHETIC-NOT-A-SHARED-CREDENTIAL", "loaded": {label: True for label in LABELS},
                      "disabled": {LABELS[1]: True, LABELS[2]: True, LABELS[3]: False},
                      "snapshot": {"contract": "ai-drain-snapshot.v1", "databaseKey": "synthetic-cluster:owned:123:public",
                                   "runningInvocations": 0, "runningItems": 0, "sessionLockHeld": True, "executor": row}}
        self.state_file = self.root / "state.json"
        self.environment = {"PATH": str(self.bin) + ":" + str(Path(sys.executable).parent) + ":/usr/bin:/bin:/usr/sbin",
                            "LANG": "en_US.UTF-8", "AI_INSTALL_PROOF_STATE": str(self.state_file),
                            "LINGGAN_RUNTIME_DIR": str(self.runtime), "LINGGAN_SUPPORT_DIR": str(self.support),
                            "LINGGAN_RUNTIME_SOURCE_DIR": str(self.source), "LINGGAN_LAUNCH_AGENTS_DIR": str(self.agents),
                            "TMPDIR": str(self.root / "tmp"), "ZDOTDIR": str(self.root / "zsh")}

    def tearDown(self):
        shutil.rmtree(self.root)

    def execute(self, mode=None, script="install.sh"):
        self.state_file.write_text(json.dumps(self.state))
        result = subprocess.run(["/bin/zsh", "-f", str(self.source / "scripts/runtime" / script), *([mode] if mode else [])],
                                env=self.environment, text=True, capture_output=True, timeout=15)
        self.state = json.loads(self.state_file.read_text())
        trace = self.root / "trace.jsonl"
        self.trace = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        return result

    def assert_no_switch(self):
        self.assertEqual(self.state["head"], "a" * 40)
        self.assertFalse(any(row[:2] == ["git", "reset"] or row[0] == "cargo" or row[:2] == ["launchctl", "bootstrap"] for row in self.trace))

    def test_two_disabled_workers_remain_off_and_ai_enabled_intent_is_preserved(self):
        result = self.execute(); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.state["head"], "b" * 40)
        bootstrapped = [plistlib.loads(Path(row[-1]).read_bytes())["Label"] for row in self.trace if row[:2] == ["launchctl", "bootstrap"]]
        self.assertEqual(bootstrapped, [LABELS[0], LABELS[3]])
        for label in (LABELS[1], LABELS[3]):
            value = plistlib.loads((self.agents / (label + ".plist")).read_bytes())
            self.assertEqual(value["ExitTimeOut"], 3616)
            self.assertEqual(value["WorkingDirectory"], str(self.runtime))
        api = plistlib.loads((self.agents / (LABELS[0] + ".plist")).read_bytes())
        self.assertEqual(api["EnvironmentVariables"]["LINGGAN_LOCAL_PORT"], "3001")
        self.assertFalse(any("-iTCP:3000" in row for row in self.trace if row[0] == "lsof"))
        permit = json.loads((self.support / "runtime-drain/worker-update-permit").read_text())
        self.assertEqual(permit["from"], "a" * 40); self.assertEqual(permit["target"], "b" * 40)
        self.assertEqual(permit["ai"]["state"], "drained")
        identity = json.loads((self.support / "runtime-identity.json").read_text()); self.assertEqual(identity["revision"], "b" * 40)
        reset = next(i for i, row in enumerate(self.trace) if row[:2] == ["git", "reset"])
        stops = [i for i, row in enumerate(self.trace) if row[:2] == ["launchctl", "bootout"]]
        self.assertTrue(all(i < reset for i in stops))
        self.assertTrue(any("linggan-ai-worker" in row for row in self.trace if row[0] == "cargo"))
        self.assertFalse(any(self.state["password"] in part for row in self.trace for part in row))

    def test_new_ai_service_is_written_disabled_and_not_bootstrapped(self):
        self.state["loaded"][LABELS[3]] = False; self.state["disabled"].pop(LABELS[3])
        self.state["snapshot"]["executor"] = None; self.state["snapshot"]["sessionLockHeld"] = False
        result = self.execute(); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertTrue(self.state["disabled"][LABELS[3]])
        self.assertFalse(self.state["loaded"][LABELS[3]])
        self.assertTrue((self.agents / (LABELS[3] + ".plist")).is_file())

    def test_migration_preparation_stops_all_four_without_reset_build_bootstrap_or_permit(self):
        result = self.execute("--prepare-migration"); self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assert_no_switch(); self.assertFalse(any(self.state["loaded"].values()))
        self.assertFalse((self.support / "runtime-drain/worker-update-permit").exists())

    def test_failed_stale_and_wrong_database_receipts_never_switch_code(self):
        for fault in ("ack_source", "ack_generation", "failed_ack", "inflight", "database"):
            with self.subTest(fault=fault):
                self.state["fault"] = fault; self.state["loaded"] = {label: True for label in LABELS}
                self.state["snapshot"]["executor"]["state"] = "running"
                self.state["snapshot"]["runningInvocations"] = 0; self.state["snapshot"]["sessionLockHeld"] = True
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()

    def test_unreadable_disabled_state_or_unknown_launchctl_state_is_not_no_worker(self):
        for key, value in (("disabled_unreadable", True), ("inspection_unreadable", LABELS[1])):
            with self.subTest(key=key):
                self.state[key] = value; (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
                self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))
                self.state.pop(key)

    def test_unreadable_process_or_port_probe_cannot_release_a_permit(self):
        for key in ("process_probe_unreadable", "port_probe_unreadable"):
            with self.subTest(key=key):
                self.state[key] = True; self.state["loaded"] = {label: True for label in LABELS}
                self.state["snapshot"]["executor"]["state"] = "running"; self.state["snapshot"]["sessionLockHeld"] = True
                (self.root / "trace.jsonl").unlink(missing_ok=True)
                result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
                self.assertFalse((self.support / "runtime-drain/worker-update-permit").exists()); self.state.pop(key)

    def test_sync_rejects_a_missing_permit_before_reset_or_build(self):
        result = self.execute(script="sync.sh"); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()

    def test_dirty_release_source_is_rejected_before_any_service_is_stopped(self):
        self.state["dirty"] = True
        result = self.execute(); self.assertNotEqual(result.returncode, 0); self.assert_no_switch()
        self.assertFalse(any(row[:2] == ["launchctl", "bootout"] for row in self.trace))


if __name__ == "__main__":
    unittest.main()
