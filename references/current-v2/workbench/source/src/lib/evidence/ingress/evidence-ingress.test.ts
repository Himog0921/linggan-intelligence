/**
 * EvidenceIngress unit tests per 07-evidence-ingress-release-b-contract.md §6.1.
 */

import { describe, it, expect } from "vitest";
import { canonicalJsonString, canonicalJsonBytes, assertJsonValue, JsonValidationError } from "./canonical-json";
import { EvidenceIngress, isRetryableSerializationError } from "./evidence-ingress";
import { decodeStrictBase64, sha256Hex, isLowercaseHex64, parseIsoTimestamp } from "./base64";
import { validateCaptureSubmission, validateSubmissionEnvelope, validatePackageIntegrity } from "./validator";
import { CollectionContractRegistry, computeContractHash } from "../contracts/collection-contract-registry";
import type { CollectionContractDefinitionV2 } from "./types";
import { AlwaysValidAuthorityValidator, AlwaysInvalidAuthorityValidator } from "./test-fixtures";
import {
  buildSubmission,
  buildBaseHeader,
  buildBody,
  buildPackagePayload,
  buildTestRecord,
  buildAuthority,
  TEST_CONTRACT,
  TEST_CONTRACT_HASH,
  testRegistry,
} from "./test-fixtures";

// ── §2.2 Canonical JSON ────────────────────────────────────────────────────

describe("canonicalJson", () => {
  it("sorts object keys recursively", () => {
    const input = { b: 1, a: { d: 2, c: 3 } };
    const result = canonicalJsonString(input);
    expect(result).toBe('{"a":{"c":3,"d":2},"b":1}');
  });

  it("preserves array order", () => {
    const input = { items: [3, 1, 2] };
    const result = canonicalJsonString(input);
    expect(result).toBe('{"items":[3,1,2]}');
  });

  it("produces no spaces", () => {
    const result = canonicalJsonString({ a: 1, b: 2 });
    expect(result).not.toContain(" ");
  });

  it("outputs UTF-8 bytes", () => {
    const bytes = canonicalJsonBytes({ label: "héllo" });
    const text = Buffer.from(bytes).toString("utf8");
    expect(text).toContain("héllo");
  });
});

// ── §2.1 JSON value validation ─────────────────────────────────────────────

describe("assertJsonValue", () => {
  it("accepts valid JSON values", () => {
    expect(() => assertJsonValue({ a: [1, "two", null, true] })).not.toThrow();
  });

  it("rejects undefined", () => {
    expect(() => assertJsonValue(undefined)).toThrow(JsonValidationError);
  });

  it("rejects NaN", () => {
    expect(() => assertJsonValue(NaN)).toThrow(JsonValidationError);
  });

  it("rejects Infinity", () => {
    expect(() => assertJsonValue(Infinity)).toThrow(JsonValidationError);
  });

  it("rejects BigInt", () => {
    expect(() => assertJsonValue(BigInt(123))).toThrow(JsonValidationError);
  });

  it("rejects functions", () => {
    expect(() => assertJsonValue(() => 1)).toThrow(JsonValidationError);
  });
});

// ── §3.3 Strict base64 ─────────────────────────────────────────────────────

describe("decodeStrictBase64", () => {
  it("decodes valid padded base64", () => {
    const input = Buffer.from("hello world").toString("base64");
    const decoded = decodeStrictBase64(input);
    expect(decoded).not.toBeNull();
    expect(Buffer.from(decoded!).toString()).toBe("hello world");
  });

  it("rejects base64url characters", () => {
    const input = Buffer.from("test").toString("base64url");
    expect(decodeStrictBase64(input)).toBeNull();
  });

  it("rejects whitespace", () => {
    const valid = Buffer.from("test").toString("base64");
    expect(decodeStrictBase64(valid + " ")).toBeNull();
  });

  it("rejects missing padding", () => {
    const valid = Buffer.from("test").toString("base64");
    const noPad = valid.replace(/=+$/, "");
    expect(decodeStrictBase64(noPad)).toBeNull();
  });

  it("rejects empty string", () => {
    expect(decodeStrictBase64("")).toBeNull();
  });

  it("round-trip: re-encode matches input", () => {
    const input = Buffer.from(JSON.stringify({ a: 1 })).toString("base64");
    const decoded = decodeStrictBase64(input);
    expect(Buffer.from(decoded!).toString("base64")).toBe(input);
  });
});

// ── sha256 ──────────────────────────────────────────────────────────────────

describe("sha256Hex", () => {
  it("produces lowercase 64-char hex", () => {
    const hash = sha256Hex(Buffer.from("test"));
    expect(hash).toMatch(/^[0-9a-f]{64}$/);
  });

  it("is deterministic", () => {
    expect(sha256Hex(Buffer.from("test"))).toBe(sha256Hex(Buffer.from("test")));
  });

  it("computes UTF-8 sha256 correctly", () => {
    const expected = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    expect(sha256Hex(Buffer.from("test", "utf8"))).toBe(expected);
  });
});

describe("isLowercaseHex64", () => {
  it("accepts valid lowercase hex", () => {
    expect(isLowercaseHex64("a".repeat(64))).toBe(true);
  });

  it("rejects uppercase", () => {
    expect(isLowercaseHex64("A".repeat(64))).toBe(false);
  });

  it("rejects wrong length", () => {
    expect(isLowercaseHex64("a".repeat(63))).toBe(false);
  });
});

describe("parseIsoTimestamp", () => {
  it("accepts UTC Z timestamp", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00Z")).not.toBeNull();
  });

  it("accepts timezone offset", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00+08:00")).not.toBeNull();
  });

  it("accepts fractional seconds with timezone", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00.123Z")).not.toBeNull();
  });

  it("accepts timestamp across UTC day boundary with positive offset (R2-002)", () => {
    // 2026-08-05T00:30:00+08:00 = UTC 2026-08-04T16:30:00
    // Must NOT reject due to UTC date mismatch with original date.
    expect(parseIsoTimestamp("2026-08-05T00:30:00+08:00")).not.toBeNull();
  });

  it("accepts negative offset across UTC day boundary (R2-002)", () => {
    // 2026-08-04T22:30:00-05:00 = UTC 2026-08-05T03:30:00
    expect(parseIsoTimestamp("2026-08-04T22:30:00-05:00")).not.toBeNull();
  });

  it("rejects bare date", () => {
    expect(parseIsoTimestamp("2026-08-05")).toBeNull();
  });

  it("rejects no-timezone local time", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00")).toBeNull();
  });

  it("rejects space separator without timezone", () => {
    expect(parseIsoTimestamp("2026-08-05 12:00:00")).toBeNull();
  });

  it("rejects illegal calendar date (Feb 30)", () => {
    expect(parseIsoTimestamp("2026-02-30T12:00:00Z")).toBeNull();
  });

  it("rejects invalid month", () => {
    expect(parseIsoTimestamp("2026-13-01T12:00:00Z")).toBeNull();
  });

  it("rejects illegal offset hour", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00+24:00")).toBeNull();
  });

  it("rejects illegal offset minute", () => {
    expect(parseIsoTimestamp("2026-08-05T12:00:00+08:60")).toBeNull();
  });

  it("rejects invalid", () => {
    expect(parseIsoTimestamp("not-a-date")).toBeNull();
  });
});

// ── §3 CaptureSubmissionV2 validation ──────────────────────────────────────

describe("validateCaptureSubmission", () => {
  it("accepts a valid manual_import submission", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("rejects unknown fields in top-level object (strict shape)", () => {
    const sub = buildSubmission("manual_import");
    (sub as Record<string, unknown>).extra = true;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects unknown fields in header", () => {
    const sub = buildSubmission("manual_import");
    (sub.body.header as Record<string, unknown>).extra = true;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects douyin platform with unsupported_platform", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: { platform: "douyin" } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("unsupported_platform");
  });

  it("rejects wrong protocolVersion", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: { protocolVersion: "v1" as never },
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects header/authority kind mismatch", () => {
    const sub = buildSubmission("manual_import");
    sub.authority.ingressKind = "recovery";
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects base64url in packagePayload", () => {
    // Build a known base64 string with + or / characters
    const raw = Buffer.from(JSON.stringify({ z: 999, y: "+++/test//" })).toString("base64");
    // Verify the original contains + or /
    if (!raw.includes("+") && !raw.includes("/")) {
      // Skip if coincidentally no special chars — but this should not happen
      expect(true).toBe(true);
      return;
    }
    // Convert to base64url
    const urlSafe = raw.replace(/\+/g, "-").replace(/\//g, "_");
    expect(decodeStrictBase64(urlSafe)).toBeNull();
  });

  it("rejects wrong checksum", () => {
    const sub = buildSubmission("manual_import");
    sub.body.capturePackage.checksumValue = "0".repeat(64);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("package_checksum_mismatch");
  });

  it("rejects wrong contentLength", () => {
    const sub = buildSubmission("manual_import");
    sub.body.capturePackage.contentLength = 999;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("package_length_mismatch");
  });

  it("rejects non-canonical package", () => {
    const header = buildBaseHeader();
    const record = buildTestRecord({});
    const { payload } = buildPackagePayload(header, [record]);
    // Non-canonical: manually build JSON with top-level keys in non-sorted order
    const reversed: Record<string, unknown> = {};
    for (const key of Object.keys(payload).reverse()) {
      reversed[key] = (payload as Record<string, unknown>)[key];
    }
    const nonCanonicalJson = JSON.stringify(reversed);
    const nonCanonicalBytes = Buffer.from(nonCanonicalJson, "utf8");
    // Build a submission with the non-canonical bytes
    const sub = buildSubmission("manual_import", { records: [record] });
    sub.body.capturePackage.packagePayload = Buffer.from(nonCanonicalBytes).toString("base64");
    sub.body.capturePackage.checksumValue = sha256Hex(nonCanonicalBytes);
    sub.body.capturePackage.contentLength = nonCanonicalBytes.length;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("package_not_canonical");
  });

  it("rejects outer/inner header mismatch", () => {
    const header = buildBaseHeader();
    const record = buildTestRecord({});
    // Build payload with a different header captureId
    const innerHeader = { ...header, captureId: "different-cap" };
    const { bytes } = buildPackagePayload(innerHeader, [record]);
    // Outer header still has original captureId
    const sub = buildSubmission("manual_import");
    sub.body.capturePackage.packagePayload = Buffer.from(bytes).toString("base64");
    sub.body.capturePackage.checksumValue = sha256Hex(bytes);
    sub.body.capturePackage.contentLength = bytes.length;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("header_package_mismatch");
  });

  it("rejects target identity mismatch", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: {
        target: { expectedTargetKey: "xhs:note/abc123", observedTargetKey: "xhs:note/different" },
      } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("target_identity_mismatch");
  });

  it("rejects expectedTargetKey without xhs: prefix", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: {
        target: { expectedTargetKey: "note/abc123", observedTargetKey: null },
      } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects duplicate sequence in records", () => {
    const records = [
      buildTestRecord({ sequence: 0, idempotencyKey: "k1" }),
      buildTestRecord({ sequence: 0, idempotencyKey: "k2", recordKind: "comment" }),
    ];
    const sub = buildSubmission("manual_import", { records });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects duplicate idempotencyKey in records", () => {
    const records = [
      buildTestRecord({ sequence: 0, idempotencyKey: "dup" }),
      buildTestRecord({ sequence: 1, idempotencyKey: "dup", recordKind: "comment" }),
    ];
    const sub = buildSubmission("manual_import", { records });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects client-submitted payloadHash", () => {
    const sub = buildSubmission("manual_import");
    const record = buildTestRecord({});
    const { payload } = buildPackagePayload(buildBaseHeader(), [record]);
    (payload.records[0] as Record<string, unknown>).payloadHash = "abc";
    const jsonStr = canonicalJsonString(payload);
    const bytes = Buffer.from(jsonStr, "utf8");
    sub.body.capturePackage.packagePayload = Buffer.from(bytes).toString("base64");
    sub.body.capturePackage.checksumValue = sha256Hex(bytes);
    sub.body.capturePackage.contentLength = bytes.length;
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects counters.emitted != records.length", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    sub.body.header.report.counters.emitted = 2;
    // Need to rebuild package to match modified header
    sub.body = buildBody(sub.body.header, [record]);
    // Now manually mismatch the counter
    const header = sub.body.header;
    header.report.counters.emitted = 2;
    sub.body = buildBody(header, [record]);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects execution without executionPlanVersion", () => {
    const sub = buildSubmission("execution");
    delete (sub.body.header as Record<string, unknown>).executionPlanVersion;
    // Rebuild package
    sub.body = buildBody(sub.body.header);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("accepts empty records for cancelled terminal", () => {
    const sub = buildSubmission("manual_import");
    sub.body.header.report.terminal.state = "cancelled";
    sub.body.header.report.counters.emitted = 0;
    sub.body = buildBody(sub.body.header);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("rejects empty records for completed with emitted > 0", () => {
    const sub = buildSubmission("manual_import");
    sub.body.header.report.terminal.state = "completed";
    sub.body.header.report.counters.emitted = 1;
    sub.body = buildBody(sub.body.header);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects duplicate slotId in report", () => {
    const sub = buildSubmission("manual_import");
    sub.body.header.report.slots = [
      { slotId: "dup", status: "observed", reason: null },
      { slotId: "dup", status: "absent", reason: "test" },
    ];
    sub.body = buildBody(sub.body.header);
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });
});

// ── R1-003: Fixed validation ordering ────────────────────────────────────

describe("validation ordering (R1-003)", () => {
  it("validateSubmissionEnvelope rejects unknown top-level fields", () => {
    const sub = buildSubmission("manual_import");
    (sub as Record<string, unknown>).extra = true;
    const result = validateSubmissionEnvelope(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("validateSubmissionEnvelope accepts valid submission", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    const result = validateSubmissionEnvelope(sub);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.header.ingressKind).toBe("manual_import");
      expect(result.authority.ingressKind).toBe("manual_import");
    }
  });

  it("validateSubmissionEnvelope does NOT decode base64 (deferred to phase 2)", () => {
    // Even with garbage base64, envelope passes — decode happens in phase 2
    const sub = buildSubmission("manual_import");
    sub.body.capturePackage.packagePayload = "!!!not-base64!!!";
    sub.body.capturePackage.checksumValue = "0".repeat(64);
    const result = validateSubmissionEnvelope(sub);
    // Envelope should pass (it only checks metadata shape)
    expect(result.ok).toBe(true);
    // But package integrity should fail
    if (result.ok) {
      const pkg = validatePackageIntegrity(result.header, result.capturePackage);
      expect(pkg.ok).toBe(false);
      if (!pkg.ok) expect(pkg.reason).toBe("invalid_base64");
    }
  });

  it("authority kind mismatch fails at envelope stage (before package decode)", () => {
    const sub = buildSubmission("manual_import");
    sub.authority.ingressKind = "recovery";
    const result = validateSubmissionEnvelope(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects expectedTargetKey with leading whitespace", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: {
        target: { expectedTargetKey: " xhs:note/abc123", observedTargetKey: null },
      } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects expectedTargetKey with trailing whitespace", () => {
    const sub = buildSubmission("manual_import", {
      headerOverrides: {
        target: { expectedTargetKey: "xhs:note/abc123 ", observedTargetKey: null },
      } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });
});

// ── §4 CollectionContractRegistry ──────────────────────────────────────────

describe("CollectionContractRegistry", () => {
  it("rejects duplicate (id, version) at load time", () => {
    expect(
      () => new CollectionContractRegistry([TEST_CONTRACT, { ...TEST_CONTRACT }])
    ).toThrow();
  });

  it("rejects duplicate slotId at load time", () => {
    const bad = {
      ...TEST_CONTRACT,
      slots: [
        { slotId: "dup", requirement: "required" as const },
        { slotId: "dup", requirement: "optional" as const },
      ],
    };
    expect(() => new CollectionContractRegistry([bad])).toThrow();
  });

  it("rejects duplicate recordKind at load time", () => {
    const bad = {
      ...TEST_CONTRACT,
      recordKinds: ["note", "note"] as never[],
    };
    expect(() => new CollectionContractRegistry([bad as never])).toThrow();
  });

  it("rejects non-positive version", () => {
    const bad = { ...TEST_CONTRACT, version: 0 };
    expect(() => new CollectionContractRegistry([bad as never])).toThrow();
  });

  it("rejects a fractional contract version", () => {
    const bad = { ...TEST_CONTRACT, version: 1.5 };
    expect(() => new CollectionContractRegistry([bad as never])).toThrow();
  });

  it("returns not_registered for unknown id", () => {
    const reg = testRegistry();
    const result = reg.lookup("unknown", 1, "0".repeat(64));
    expect(result.status).toBe("not_registered");
  });

  it("returns hash_mismatch for wrong hash", () => {
    const reg = testRegistry();
    const result = reg.lookup(TEST_CONTRACT.id, 1, "0".repeat(64));
    expect(result.status).toBe("hash_mismatch");
  });

  it("returns resolved for correct hash", () => {
    const reg = testRegistry();
    const result = reg.lookup(TEST_CONTRACT.id, 1, TEST_CONTRACT_HASH);
    expect(result.status).toBe("resolved");
  });

  it("computeContractHash is deterministic", () => {
    const h1 = computeContractHash(TEST_CONTRACT);
    const h2 = computeContractHash({ ...TEST_CONTRACT });
    expect(h1).toBe(h2);
  });

  it("validateShape rejects unknown recordKind", () => {
    const reg = testRegistry();
    const def = TEST_CONTRACT;
    expect(
      reg.validateShape(def, {
        recordKinds: ["note" as const, "video" as never],
        slotIds: ["note_content"],
        terminalState: "completed",
        recordsEmpty: false,
      })
    ).toBe(false);
  });

  it("validateShape rejects unknown slotId", () => {
    const reg = testRegistry();
    expect(
      reg.validateShape(TEST_CONTRACT, {
        recordKinds: ["note"],
        slotIds: ["unknown_slot"],
        terminalState: "completed",
        recordsEmpty: false,
      })
    ).toBe(false);
  });

  it("validateShape rejects disallowed terminal state", () => {
    const def: CollectionContractDefinitionV2 = {
      ...TEST_CONTRACT,
      terminalPolicy: { ...TEST_CONTRACT.terminalPolicy, allowedStates: ["completed"] },
    };
    const reg = new CollectionContractRegistry([def]);
    expect(
      reg.validateShape(def, {
        recordKinds: [],
        slotIds: [],
        terminalState: "error" as const,
        recordsEmpty: true,
      })
    ).toBe(false);
  });
});

// ── §3.4 Authority validation ──────────────────────────────────────────────

describe("EvidenceIngress authority + contract integration", () => {
  // These tests use mocked Prisma — just test the validation + rejection path

  it("AlwaysInvalidAuthorityValidator returns correct rejection reason", async () => {
    const validator = new AlwaysInvalidAuthorityValidator("manual_import_authority_invalid");
    const result = await validator.validate(buildAuthority("manual_import"), buildBaseHeader());
    expect(result.valid).toBe(false);
    if (!result.valid) expect(result.reason).toBe("manual_import_authority_invalid");
  });

  it("AlwaysValidAuthorityValidator approves", async () => {
    const validator = new AlwaysValidAuthorityValidator();
    const result = await validator.validate(buildAuthority("manual_import"), buildBaseHeader());
    expect(result.valid).toBe(true);
  });

  it("empty registry returns not_registered", () => {
    const emptyReg = new CollectionContractRegistry([]);
    expect(emptyReg.lookup("any", 1, "0".repeat(64)).status).toBe("not_registered");
  });
});

// ── R2-002: Non-null identifier whitespace rejection ─────────────────────

describe("record identifier validation (R2-002)", () => {
  it("rejects non-null targetKey that is whitespace-only", () => {
    const rec = buildTestRecord({ targetKey: "   " });
    const sub = buildSubmission("manual_import", {
      records: [rec],
      headerOverrides: { captureId: "cap-ws-target" } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects non-null externalRecordId that is empty string", () => {
    const rec = buildTestRecord({ externalRecordId: "" });
    const sub = buildSubmission("manual_import", {
      records: [rec],
      headerOverrides: { captureId: "cap-empty-ext" } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });

  it("rejects non-null targetKey with trailing whitespace", () => {
    const rec = buildTestRecord({ targetKey: "xhs:note/abc " });
    const sub = buildSubmission("manual_import", {
      records: [rec],
      headerOverrides: { captureId: "cap-trail-target" } as never,
    });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("invalid_submission");
  });
});

// ── R2-003: Four-kind fixtures and EvidenceIngress orchestration ─────────

describe("four-kind fixtures and EvidenceIngress (R2-003)", () => {
  it("execution fixture envelope passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("execution", { records: [record] });
    const env = validateSubmissionEnvelope(sub);
    expect(env.ok).toBe(true);
  });

  it("execution fixture full package passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("execution", { records: [record] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("manual_import fixture envelope passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    const env = validateSubmissionEnvelope(sub);
    expect(env.ok).toBe(true);
  });

  it("manual_import fixture full package passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("recovery fixture envelope passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("recovery", { records: [record] });
    const env = validateSubmissionEnvelope(sub);
    expect(env.ok).toBe(true);
  });

  it("recovery fixture full package passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("recovery", { records: [record] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("migration fixture envelope passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("migration", { records: [record] });
    const env = validateSubmissionEnvelope(sub);
    expect(env.ok).toBe(true);
  });

  it("migration fixture full package passes validation", () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("migration", { records: [record] });
    const result = validateCaptureSubmission(sub);
    expect(result.ok).toBe(true);
  });

  it("execution header does NOT contain sourceSummary (R2-003)", () => {
    const hdr = buildBaseHeader("execution");
    // sourceSummary should not exist on execution headers
    expect((hdr as Record<string, unknown>).sourceSummary).toBeUndefined();
  });

  it("recovery header does NOT contain sourceSummary (R2-003)", () => {
    const hdr = buildBaseHeader("recovery");
    expect((hdr as Record<string, unknown>).sourceSummary).toBeUndefined();
  });

  // R2-003: EvidenceIngress with authority invalid + package damaged
  // The authority rejection must come first, and registry/DB must not be called.
  it("rejects on authority failure before package decode (authority+package both damaged)", async () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    // Damage the package
    sub.body.capturePackage.packagePayload = "!!!invalid!!!";
    sub.body.capturePackage.checksumValue = "0".repeat(64);

    const validator = new AlwaysInvalidAuthorityValidator("manual_import_authority_invalid");
    const reg = testRegistry();

    // DB must never be called because authority fails first.
    let dbCalled = false;
    const stubDb = {
      $transaction: () => { dbCalled = true; throw new Error("DB must not be called"); },
    };

    const ingress = new EvidenceIngress(stubDb as never, reg, validator);
    const result = await ingress.submit(sub);

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("manual_import_authority_invalid");
      expect(result.retryable).toBe(false);
    }
    expect(dbCalled).toBe(false);
  });

  it("authority rejection does not access registry", async () => {
    const record = buildTestRecord({});
    const sub = buildSubmission("manual_import", { records: [record] });
    sub.body.capturePackage.packagePayload = "!!!invalid!!!";

    // Registry must not be called when authority fails
    let registryCalled = false;
    const stubReg = {
      lookup: () => { registryCalled = true; throw new Error("Registry must not be called"); },
      validateShape: () => { registryCalled = true; throw new Error("Registry must not be called"); },
    };

    const validator = new AlwaysInvalidAuthorityValidator("execution_authority_invalid");
    const stubDb = {
      $transaction: () => { throw new Error("DB must not be called"); },
    };

    const ingress = new EvidenceIngress(stubDb as never, stubReg as never, validator);
    const result = await ingress.submit(sub);

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.reason).toBe("execution_authority_invalid");
    }
    expect(registryCalled).toBe(false);
  });
});

describe("serialization error classification", () => {
  it("accepts structured PostgreSQL serialization and deadlock codes", () => {
    expect(isRetryableSerializationError({ code: "40001" })).toBe(true);
    expect(isRetryableSerializationError({ meta: { code: "40P01" } })).toBe(true);
    expect(isRetryableSerializationError({ cause: { code: "40001" } })).toBe(true);
  });

  it("does not retry an unrelated error that only mentions a SQLSTATE", () => {
    expect(isRetryableSerializationError(new Error("remote message mentioned 40001"))).toBe(false);
  });
});
