// @vitest-environment node

import { describe, expect, it } from "vitest";

import { XHS_LIST_SCAN, XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";
import {
  evaluateXhsContract,
  type XhsEvaluationMember,
} from "./xhs-derived-contract";

const CURRENT_NOTE: XhsEvaluationMember = {
  recordId: "rr-1",
  memberKind: "current",
  runId: "nr-1",
  attemptNumber: 1,
  status: "normalized",
  adapterId: "xhs.note",
  adapterVersion: "2.0.0",
  canonicalSchemaVersion: "xhs.canonical/2",
  inputHash: "a".repeat(64),
  outputHash: "b".repeat(64),
};

describe("XHS ContractEvaluation", () => {
  it("matches the approved evaluationInput hash vector", () => {
    const result = evaluateXhsContract({
      snapshot: { workspaceId: "ws-fixture", rawSnapshotId: "rs-1" },
      eligibility: { lifecycleStatus: "ACTIVE", integrityStatus: "verified" },
      contract: XHS_LIST_SCAN,
      terminal: { state: "completed", reason: "limit_reached", retryable: false },
      slots: [{ slotId: "note_list", status: "observed", reason: null }],
      members: [CURRENT_NOTE],
    });

    expect(result.evaluationInputHash).toBe(
      "a661d60500a8f93c169eae2c49a1a04b1e30858fe660c9fe197cbb2be1befc9a",
    );
    expect(result).toMatchObject({
      decision: "accepted",
      completeness: "full",
      rejectionCode: null,
      rejectionReason: null,
    });
  });

  it("uses the frozen rejection priority and stable record identities", () => {
    const result = evaluateXhsContract({
      snapshot: { workspaceId: "workspace-1", rawSnapshotId: "snapshot-1" },
      eligibility: { lifecycleStatus: "ARCHIVED", integrityStatus: "verified" },
      contract: XHS_NOTE_DETAIL,
      terminal: { state: "blocked", reason: "login_required", retryable: true },
      slots: [
        { slotId: "comments", status: "observed", reason: null },
        { slotId: "note", status: "absent", reason: "not_found" },
      ],
      members: [
        {
          ...CURRENT_NOTE,
          recordId: "record-b",
          memberKind: "latest_rejected_attempt",
          status: "rejected",
        },
        { ...CURRENT_NOTE, recordId: "record-a", runId: "run-a" },
      ],
    });

    expect(result).toMatchObject({
      decision: "rejected",
      completeness: "not_applicable",
      rejectionCode: "terminal_not_completed",
      rejectionReason:
        '{"code":"terminal_not_completed","details":{"state":"blocked"}}',
    });
  });

  it("accepts partial when required slots are satisfied and an optional slot is absent", () => {
    const result = evaluateXhsContract({
      snapshot: { workspaceId: "workspace-1", rawSnapshotId: "snapshot-1" },
      eligibility: { lifecycleStatus: "ACTIVE", integrityStatus: "verified" },
      contract: XHS_NOTE_DETAIL,
      terminal: { state: "completed", reason: "source_exhausted", retryable: false },
      slots: [
        { slotId: "note", status: "observed", reason: null },
        { slotId: "comments", status: "absent", reason: "not_found" },
      ],
      members: [CURRENT_NOTE],
    });

    expect(result).toMatchObject({
      decision: "accepted",
      completeness: "partial",
      rejectionCode: null,
      rejectionReason: null,
    });
  });
});
