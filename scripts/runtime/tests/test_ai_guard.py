"""Pure negative boundaries and serialized file contracts; no database or service operations."""
import copy
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import unittest
import uuid

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ai_guard as guard

SOURCE = "a" * 40
TARGET = "b" * 40


class MigrationRegistry(unittest.TestCase):
    def test_every_migration_has_one_matching_runtime_registration(self):
        root = Path(__file__).resolve().parents[3]
        source = (root / "scripts/local-runtime.sh").read_text()
        registrations = re.findall(
            r'apply_migration_once "([^"]+)" "\$project_root/database/migrations/([^"]+)"',
            source,
        )
        expected = {path.name for path in (root / "database/migrations").glob("*.sql")}
        self.assertEqual(expected, {name for _, name in registrations})
        self.assertEqual(len(expected), len(registrations))
        self.assertTrue(all(key == Path(name).stem for key, name in registrations))


def example():
    row = {"instanceRef": str(uuid.uuid4()), "generation": "4", "processId": 23456,
           "sourceRevision": SOURCE, "state": "drained"}
    db = {"contract": "ai-drain-snapshot.v1", "databaseKey": "synthetic-cluster:owned:1234:synthetic_schema",
          "runningInvocations": 0, "runningItems": 0, "sessionLockHeld": False, "executor": row}
    receipt = {"state": "drained", "processId": row["processId"],
               "receipt": {key: value for key, value in row.items() if key != "state"}}
    return db, receipt


class GuardContracts(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[3] / "artifacts/private/ai-gen1-pr1b" / ("install-proof." + str(uuid.uuid4()))
        self.root.mkdir(mode=0o700)

    def tearDown(self):
        import shutil
        shutil.rmtree(self.root)

    def test_database_terminal_receipt_requires_every_identity_and_zero_inflight(self):
        db, receipt = example()
        self.assertEqual(guard.ack(receipt, 23456, SOURCE, db)["state"], "drained")
        mutations = [("processId", True), ("processId", 23457), ("generation", "04"),
                     ("generation", 4), ("generation", str(2**63)), ("sourceRevision", "unknown"),
                     ("sourceRevision", SOURCE[:8]), ("instanceRef", str(uuid.uuid4()))]
        for key, value in mutations:
            with self.subTest(key=key, value=value):
                changed = copy.deepcopy(receipt); changed["receipt"][key] = value
                with self.assertRaises((ValueError, TypeError)):
                    guard.ack(changed, 23456, SOURCE, db)
        for key, value in [("runningInvocations", 1), ("runningItems", 1), ("sessionLockHeld", True),
                           ("runningItems", None), ("runningInvocations", "0"), ("runningItems", False)]:
            with self.subTest(key=key, value=value):
                changed = copy.deepcopy(db); changed[key] = value
                with self.assertRaises(ValueError):
                    guard.ack(receipt, 23456, SOURCE, changed)

    def test_no_ai_proof_cannot_hide_unknown_reservations_or_become_a_drained_claim(self):
        db, _ = example(); db["executor"] = None
        proof = guard.no_ai(db)
        self.assertEqual(proof["state"], "not_running")
        changed = copy.deepcopy(db); changed["runningInvocations"] = 1
        with self.assertRaises(ValueError): guard.proof(proof, SOURCE, changed)
        changed = copy.deepcopy(db); changed["databaseKey"] = "another-cluster:owned"
        with self.assertRaises(ValueError): guard.proof(proof, SOURCE, changed)

    def test_disabled_dictionary_preserves_workers_and_starts_new_ai_disabled(self):
        result = guard.disabled('disabled services = {\n "com.linggan-intelligence.patrol-worker" => true\n "com.linggan-intelligence.media-worker" => true\n}')
        self.assertTrue(result[guard.LABELS[1]])
        self.assertTrue(result[guard.LABELS[2]])
        self.assertTrue(result[guard.LABELS[3]])
        self.assertFalse(result[guard.LABELS[0]])
        self.assertFalse(guard.disabled('{\n "com.linggan-intelligence.ai-executor" => false\n}')[guard.LABELS[3]])
        native = guard.disabled('disabled services = {\n "com.linggan-intelligence.local-runtime" => enabled\n "com.linggan-intelligence.patrol-worker" => disabled\n "com.linggan-intelligence.media-worker" => disabled\n "other.service" => enabled\n}')
        self.assertEqual(native, dict(zip(guard.LABELS, (False, True, True, True))))
        self.assertFalse(guard.disabled('{\n "com.linggan-intelligence.ai-executor" => enabled\n}')[guard.LABELS[3]])
        self.assertTrue(guard.disabled('{\n "com.linggan-intelligence.ai-executor" => disabled\n}')[guard.LABELS[3]])
        for text in ["permission denied", "unreadable {}", '{\n "x" => 0\n}', '{\n "x" => true\n "x" => false\n}']:
            with self.subTest(text=text), self.assertRaises(ValueError): guard.disabled(text)

    def test_resource_grace_uses_the_larger_frozen_contract_and_rejects_missing_unknown_or_boolean(self):
        current = Path(__file__).resolve().parents[3] / "crates/contracts/src/ai_limits.json"
        self.assertEqual(guard.grace([str(current)]), 3616)
        old = self.root / "old.json"
        value = guard.read(str(current)); value["maxTimeoutSeconds"] = 7200; guard.atomic(old, value)
        self.assertEqual(guard.grace([str(current), str(old)]), 7216)
        value["maxTimeoutSeconds"] = True; guard.atomic(old, value)
        with self.assertRaises(ValueError): guard.grace([str(current), str(old)])
        with self.assertRaises((ValueError, OSError)): guard.grace([str(self.root / "absent.json")])

    def test_json_is_closed_duplicate_keys_are_rejected_and_files_are_atomic_private(self):
        with self.assertRaises(ValueError): guard.loads('{"state":"drained","state":"failed"}')
        db, receipt = example(); receipt["secret"] = "not-allowed"
        with self.assertRaises(ValueError): guard.ack(receipt, 23456, SOURCE, db)
        file = self.root / "metadata.json"; guard.atomic(file, {"state": "first"}); guard.atomic(file, {"state": "second"})
        self.assertEqual(guard.read(file), {"state": "second"})
        self.assertEqual(file.stat().st_mode & 0o777, 0o600)
        self.assertEqual([p.name for p in self.root.iterdir()], ["metadata.json"])
        link = self.root / "link.json"; link.symlink_to(file)
        with self.assertRaises(ValueError): guard.read(link)

    def test_plist_escapes_paths_and_retains_log_names_and_derived_timeout(self):
        file = self.root / "job.plist"; runtime = str(self.root / "Runtime & <source>")
        guard.plist([str(file), guard.LABELS[3], "linggan-ai-worker", runtime, str(self.root),
                     str(self.root / "logs"), "ai-worker", "3616", ""])
        value = plistlib.loads(file.read_bytes())
        self.assertEqual(value["WorkingDirectory"], runtime)
        self.assertEqual(value["ExitTimeOut"], 3616)
        self.assertEqual(value["StandardOutPath"], str(self.root / "logs/ai-worker.out.log"))
        self.assertEqual(value["EnvironmentVariables"]["LINGGAN_AI_DRAIN_ACK_PATH"], str(self.root / "runtime-drain/ai-drain-ack"))

    def test_signal_zero_checks_a_real_owned_pid_without_sending_a_signal(self):
        helper = Path(__file__).resolve().parents[1] / "ai_guard.py"
        child = subprocess.Popen(["/usr/bin/true"]); pid = child.pid; child.wait()
        self.assertEqual(subprocess.run([sys.executable, "-B", str(helper), "process-gone", str(pid)], capture_output=True).returncode, 0)
        self.assertNotEqual(subprocess.run([sys.executable, "-B", str(helper), "process-gone", str(os.getpid())], capture_output=True).returncode, 0)

    def test_permit_binds_source_target_database_and_current_instance(self):
        db, receipt = example(); flags = {label: True for label in guard.LABELS}
        proof_file = self.root / "proof.json"; flag_file = self.root / "flags.json"; permit = self.root / "permit.json"
        guard.atomic(proof_file, guard.ack(receipt, 23456, SOURCE, db)); guard.atomic(flag_file, flags)
        helper = Path(__file__).resolve().parents[1] / "ai_guard.py"
        run = lambda *args, value=db: subprocess.run([sys.executable, "-B", str(helper), *map(str, args)], input=json.dumps(value), text=True, capture_output=True)
        self.assertEqual(run("permit", permit, SOURCE, TARGET, "drained", proof_file, flag_file).returncode, 0)
        self.assertEqual(run("check-permit", permit, SOURCE, TARGET).returncode, 0)
        self.assertNotEqual(run("check-permit", permit, SOURCE, "c" * 40).returncode, 0)
        changed = copy.deepcopy(db); changed["executor"]["generation"] = "5"
        self.assertNotEqual(run("check-permit", permit, SOURCE, TARGET, value=changed).returncode, 0)

    def test_local_shutdown_proof_cannot_authorize_a_release(self):
        db, receipt = example(); db["executor"]["sourceRevision"] = "unknown"; receipt["receipt"]["sourceRevision"] = "unknown"
        file = self.root / "local-ack.json"; guard.atomic(file, receipt)
        helper = Path(__file__).resolve().parents[1] / "ai_guard.py"
        result = subprocess.run([sys.executable, "-B", str(helper), "check-local-ack", str(file), "23456"],
                                input=json.dumps(db), text=True, capture_output=True)
        self.assertEqual(result.returncode, 0)
        value = json.loads(result.stdout); self.assertEqual(value["state"], "local_drained")
        with self.assertRaises(ValueError): guard.proof(value, SOURCE, db)
        with self.assertRaises(ValueError): guard.ack(receipt, 23456, SOURCE, db)


if __name__ == "__main__":
    unittest.main()
