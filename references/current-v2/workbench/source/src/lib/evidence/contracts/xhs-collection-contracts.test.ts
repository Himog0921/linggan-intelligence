/**
 * Tests for 6 real XHS CollectionContract definitions (B1-B-03-R1 §5-6).
 */

import { describe, it, expect } from "vitest";
import { CollectionContractRegistry, computeContractHash, validateContractDefinition } from "./collection-contract-registry";
import {
  XHS_LIST_SCAN,
  XHS_NOTE_DETAIL,
  XHS_NOTE_FULL,
  XHS_COMMENT_PROBE,
  XHS_AUTHOR_PROFILE,
  XHS_AUTHOR_LINKS,
  XHS_COLLECTION_CONTRACTS,
  XHS_FIXTURE_PACKAGE_HASHES,
} from "./xhs-collection-contracts";
import { validateCaptureSubmission } from "../ingress/validator";
import { buildSubmission, buildTestRecord, buildBody } from "../ingress/test-fixtures";
import type { CollectionContractDefinitionV2 } from "../ingress/types";

const ALL = [
  XHS_LIST_SCAN,
  XHS_NOTE_DETAIL,
  XHS_NOTE_FULL,
  XHS_COMMENT_PROBE,
  XHS_AUTHOR_PROFILE,
  XHS_AUTHOR_LINKS,
] as const;

const reg = new CollectionContractRegistry([...XHS_COLLECTION_CONTRACTS]);

// ── §2: 6 contracts exist, each validated ────────────────────────────────

describe("6 XHS contracts (08 §2)", () => {
  it("has exactly 6 contracts", () => {
    expect(XHS_COLLECTION_CONTRACTS).toHaveLength(6);
  });

  it("all 6 contract hashes are distinct", () => {
    const hashes = ALL.map((c) => computeContractHash(c));
    expect(new Set(hashes).size).toBe(6);
  });

  it("all 6 contract hashes are lowercase 64-char hex", () => {
    for (const c of ALL) {
      expect(computeContractHash(c)).toMatch(/^[0-9a-f]{64}$/);
    }
  });

  it("has six independently locked fixture package hashes", () => {
    expect(Object.keys(XHS_FIXTURE_PACKAGE_HASHES).sort()).toEqual(ALL.map((c) => c.id).sort());
    for (const hash of Object.values(XHS_FIXTURE_PACKAGE_HASHES)) expect(hash).toMatch(/^[0-9a-f]{64}$/);
  });

  for (const c of ALL) {
    const id = c.id;
    it(`${id}: version=2 with source contract hashes`, () => {
      expect(c.version).toBe(2);
      expect(c.sourceContracts?.recordPayload.schemaVersion).toBe("xhs.record-payload/v2");
      expect(c.sourceContracts?.mediaInventory.schemaVersion).toBe("xhs.media-inventory/v2");
      expect(c.sourceContracts?.recordPayload.contractHash).toMatch(/^[0-9a-f]{64}$/);
      expect(c.sourceContracts?.mediaInventory.contractHash).toMatch(/^[0-9a-f]{64}$/);
    });
    it(`${id}: platforms=["xhs"]`, () => expect(c.platforms).toEqual(["xhs"]));
    it(`${id}: allowEmptyRecords=true`, () => expect(c.terminalPolicy.allowEmptyRecords).toBe(true));
    it(`${id}: allowedStates=[completed,blocked,cancelled,error]`, () =>
      expect(c.terminalPolicy.allowedStates).toEqual(["completed", "blocked", "cancelled", "error"]));
    it(`${id}: hash deterministic`, () =>
      expect(computeContractHash(c)).toBe(computeContractHash({ ...c })));
    it(`${id}: resolves in registry`, () =>
      expect(reg.lookup(c.id, c.version, computeContractHash(c)).status).toBe("resolved"));
  }

  // ── Per-contract assertions ─────────────────────────────────────────
  it("list-scan: recordKinds=[note], note_list required, metadata_only", () => {
    expect(XHS_LIST_SCAN.recordKinds).toEqual(["note"]);
    expect(XHS_LIST_SCAN.slots).toEqual([{ slotId: "note_list", requirement: "required" }]);
    expect(XHS_LIST_SCAN.mediaPolicy).toBe("metadata_only");
  });

  it("note-detail: recordKinds=[note,comment], note required + comments conditional", () => {
    expect(XHS_NOTE_DETAIL.recordKinds).toEqual(["note", "comment"]);
    expect(XHS_NOTE_DETAIL.slots).toEqual([
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "conditional" },
    ]);
  });

  it("note-full: recordKinds=[note,comment], note required + comments required", () => {
    expect(XHS_NOTE_FULL.recordKinds).toEqual(["note", "comment"]);
    expect(XHS_NOTE_FULL.slots).toEqual([
      { slotId: "note", requirement: "required" },
      { slotId: "comments", requirement: "required" },
    ]);
  });

  it("comment-probe: recordKinds=[comment], comments required, not_required", () => {
    expect(XHS_COMMENT_PROBE.recordKinds).toEqual(["comment"]);
    expect(XHS_COMMENT_PROBE.slots).toEqual([{ slotId: "comments", requirement: "required" }]);
    expect(XHS_COMMENT_PROBE.mediaPolicy).toBe("not_required");
  });

  it("author-profile: recordKinds=[author,note], author required + note_list optional", () => {
    expect(XHS_AUTHOR_PROFILE.recordKinds).toEqual(["author", "note"]);
    expect(XHS_AUTHOR_PROFILE.slots).toEqual([
      { slotId: "author", requirement: "required" },
      { slotId: "note_list", requirement: "optional" },
    ]);
  });

  it("author-links: recordKinds=[note], note_links required, not_required", () => {
    expect(XHS_AUTHOR_LINKS.recordKinds).toEqual(["note"]);
    expect(XHS_AUTHOR_LINKS.slots).toEqual([{ slotId: "note_links", requirement: "required" }]);
    expect(XHS_AUTHOR_LINKS.mediaPolicy).toBe("not_required");
  });
});

// ── §5: Negative tests ──────────────────────────────────────────────────

describe("registry negative cases", () => {
  it("rejects unknown id", () => {
    expect(reg.lookup("xhs.unknown", 1, "0".repeat(64)).status).toBe("not_registered");
  });

  it("rejects unknown version", () => {
    expect(reg.lookup("xhs.list-scan", 99, "0".repeat(64)).status).toBe("not_registered");
  });

  it("rejects the pre-source-binding v1 contract", () => {
    expect(reg.lookup("xhs.list-scan", 1, "0".repeat(64)).status).toBe("not_registered");
  });

  it("rejects hash_mismatch", () => {
    const r = reg.lookup("xhs.list-scan", 2, "0".repeat(64));
    expect(r.status).toBe("hash_mismatch");
    if (r.status === "hash_mismatch") {
      expect(r.expectedHash).toBe(computeContractHash(XHS_LIST_SCAN));
    }
  });

  it("rejects unallowed recordKind", () => {
    expect(reg.validateShape(XHS_LIST_SCAN, {
      recordKinds: ["comment" as never],
      slotIds: ["note_list"],
      terminalState: "completed",
      recordsEmpty: false,
    })).toBe(false);
  });

  it("rejects unregistered slotId", () => {
    expect(reg.validateShape(XHS_NOTE_DETAIL, {
      recordKinds: ["note"],
      slotIds: ["unknown_slot"],
      terminalState: "completed",
      recordsEmpty: false,
    })).toBe(false);
  });

  it("rejects disallowed terminal state", () => {
    const strict: CollectionContractDefinitionV2 = {
      ...XHS_LIST_SCAN,
      terminalPolicy: { allowedStates: ["completed"], allowEmptyRecords: true },
    };
    const r = new CollectionContractRegistry([strict]);
    expect(r.validateShape(strict, {
      recordKinds: [],
      slotIds: [],
      terminalState: "error",
      recordsEmpty: true,
    })).toBe(false);
  });

  it("rejects empty records when allowEmptyRecords=false", () => {
    const strict: CollectionContractDefinitionV2 = {
      ...XHS_LIST_SCAN,
      terminalPolicy: { allowedStates: ["completed"], allowEmptyRecords: false },
    };
    const r = new CollectionContractRegistry([strict]);
    expect(r.validateShape(strict, {
      recordKinds: [],
      slotIds: [],
      terminalState: "completed",
      recordsEmpty: true,
    })).toBe(false);
  });

  it("rejects completed + emitted>0 + empty records at validator level (07 §3.3)", () => {
    const sub = buildSubmission("manual_import");
    sub.body.header.report.terminal.state = "completed";
    sub.body.header.report.counters.emitted = 5;
    // Rebuild body with 0 records but counters.emitted=5
    const broken = buildBody(sub.body.header, []);
    sub.body = broken;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects V1 'media' record submitted as V2 RawRecord", () => {
    // "media" is not a valid RecordKindV2 — rejected at structure validation.
    const rec = buildTestRecord({ recordKind: "media" as never });
    const sub = buildSubmission("manual_import", { records: [rec] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects duplicate (id, version)", () => {
    expect(() => new CollectionContractRegistry([XHS_LIST_SCAN, { ...XHS_LIST_SCAN }])).toThrow();
  });

  it("rejects duplicate slotId at load time", () => {
    const bad = { ...XHS_LIST_SCAN, slots: [
      { slotId: "dup", requirement: "required" as const },
      { slotId: "dup", requirement: "optional" as const },
    ]};
    expect(() => new CollectionContractRegistry([bad])).toThrow();
  });

  it("rejects duplicate recordKind at load time", () => {
    const bad = { ...XHS_NOTE_DETAIL, recordKinds: ["note", "note"] };
    expect(() => new CollectionContractRegistry([bad as CollectionContractDefinitionV2])).toThrow();
  });

  it("rejects non-positive version", () => {
    const bad = { ...XHS_LIST_SCAN, version: 0 };
    expect(() => validateContractDefinition(bad as CollectionContractDefinitionV2)).toThrow();
  });

  it("rejects non-XHS platform", () => {
    const bad = { ...XHS_LIST_SCAN, platforms: ["douyin" as never] };
    expect(() => validateContractDefinition(bad as CollectionContractDefinitionV2)).toThrow();
  });

  it("no contract produces media recordKind (media → artifact)", () => {
    for (const c of ALL) {
      expect(c.recordKinds).not.toContain("media");
    }
  });

  it("no contract produces metric recordKind (V2 first phase)", () => {
    for (const c of ALL) {
      expect(c.recordKinds).not.toContain("metric");
    }
  });
});

// ── Contract hash cross-reference table ─────────────────────────────────

describe("contract hash table", () => {
  it("has stable hashes for every contract", () => {
    const hashes = ALL.map((c) => ({ id: c.id, version: c.version, contractHash: computeContractHash(c) }));
    expect(hashes).toHaveLength(6);
    expect(hashes.every(({ version, contractHash }) => version === 2 && /^[0-9a-f]{64}$/.test(contractHash))).toBe(true);
    expect(new Set(hashes.map(({ contractHash }) => contractHash)).size).toBe(6);
  });
});
